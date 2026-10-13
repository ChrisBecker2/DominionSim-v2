//! A sampling profiler for a release build with line tables (`debug = 1`, as the workspace profile has): no admin rights, no ETW.
//!
//!   winprof PROGRAM [ARGS...]
//!
//! Runs PROGRAM with its stdout discarded; about every 300 microseconds it suspends each thread of the program, reads the
//! instruction pointer and resumes it. At the end it prints the samples that fell inside the program's own image (idle worker
//! threads sit in ntdll and are left out) by function and by source line, resolved through the program's .pdb (searched next to
//! the exe). The percentages are of those samples. Set `RAYON_NUM_THREADS=1` for a profile of one thread, `PROF_TID=<id>` to
//! count only one thread. Inlined code is attributed to the source line it came from, and a long dependency chain shows up on the
//! instruction that waits for it. Sampling with many threads is slow (each round suspends all of them): use few threads and a
//! run of several seconds.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Diagnostics::Debug::*;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
use windows_sys::Win32::System::Threading::*;

#[repr(C, align(16))]
struct Context(CONTEXT);

/// `GetExitCodeProcess` reports this while the process runs.
const STILL_ACTIVE: u32 = 259;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: winprof PROGRAM [ARGS...]");
        std::process::exit(2);
    }
    let child = std::process::Command::new(&args[0]).args(&args[1..]).stdout(std::process::Stdio::null()).spawn().expect("cannot start the program");
    let pid = child.id();
    let only_tid: Option<u32> = std::env::var("PROF_TID").ok().and_then(|v| v.parse().ok());

    // Symbols: load the main module's pdb (found next to the exe) once the loader has mapped it.
    let process = unsafe { OpenProcess(PROCESS_ALL_ACCESS, 0, pid) };
    let mut image = (0u64, 0u64);
    unsafe {
        SymSetOptions(SYMOPT_LOAD_LINES | SYMOPT_UNDNAME);
        let dir = CString::new(std::path::Path::new(&args[0]).parent().and_then(|p| p.to_str()).unwrap_or(".")).unwrap();
        SymInitialize(process, dir.as_ptr() as *const u8, 0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid);
        let mut module: MODULEENTRY32 = std::mem::zeroed();
        module.dwSize = std::mem::size_of::<MODULEENTRY32>() as u32;
        if Module32First(snap, &mut module) != 0 {
            let base = module.modBaseAddr as u64;
            image = (base, base + module.modBaseSize as u64);
            let path = CString::new(args[0].clone()).unwrap();
            SymLoadModuleEx(process, std::ptr::null_mut(), path.as_ptr() as *const u8, std::ptr::null(), base, module.modBaseSize, std::ptr::null(), 0);
        }
        CloseHandle(snap);
    }

    let mut samples: HashMap<u64, u32> = HashMap::new();
    let mut total = 0u32;
    loop {
        let mut code = 0u32;
        unsafe { GetExitCodeProcess(process, &mut code) };
        if code != STILL_ACTIVE {
            break;
        }
        for tid in threads_of(pid) {
            unsafe {
                let thread = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, tid);
                if thread.is_null() {
                    continue;
                }
                if SuspendThread(thread) != u32::MAX {
                    let mut context: Context = std::mem::zeroed();
                    context.0.ContextFlags = CONTEXT_CONTROL_AMD64;
                    if GetThreadContext(thread, &mut context.0) != 0 {
                        let ip = context.0.Rip;
                        if ip >= image.0 && ip < image.1 && only_tid.map_or(true, |t| t == tid) {
                            *samples.entry(ip).or_default() += 1;
                            total += 1;
                        }
                    }
                    ResumeThread(thread);
                }
                CloseHandle(thread);
            }
        }
        // A short busy wait: `sleep` rounds up to the timer tick (15 ms).
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_micros(300) {
            std::hint::spin_loop();
        }
    }
    eprintln!("{total} samples");

    let mut by_function: HashMap<String, u32> = HashMap::new();
    let mut by_line: HashMap<String, u32> = HashMap::new();
    for (&ip, &n) in &samples {
        let (name, line) = unsafe { symbolize(process, ip) };
        *by_line.entry(format!("{line}  [{name}]")).or_default() += n;
        *by_function.entry(name).or_default() += n;
    }
    let show = |m: &HashMap<String, u32>, top: usize| {
        let mut rows: Vec<_> = m.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1));
        for (what, n) in rows.into_iter().take(top) {
            println!("{:5.1}% {}", *n as f64 * 100.0 / total.max(1) as f64, what);
        }
    };
    println!("--- functions");
    show(&by_function, 40);
    println!("--- lines");
    show(&by_line, 60);
}

fn threads_of(pid: u32) -> Vec<u32> {
    let mut tids = vec![];
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snap, &mut entry) != 0 {
            loop {
                if entry.th32OwnerProcessID == pid {
                    tids.push(entry.th32ThreadID);
                }
                if Thread32Next(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    tids
}

/// The function name and `file:line` of an address ("?" when the pdb has none).
unsafe fn symbolize(process: HANDLE, ip: u64) -> (String, String) {
    let mut buf = vec![0u8; std::mem::size_of::<SYMBOL_INFO>() + 512];
    let info = buf.as_mut_ptr() as *mut SYMBOL_INFO;
    (*info).SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
    (*info).MaxNameLen = 500;
    let mut displacement = 0u64;
    let name = if SymFromAddr(process, ip, &mut displacement, info) != 0 {
        let bytes = std::slice::from_raw_parts((*info).Name.as_ptr() as *const u8, (*info).NameLen as usize);
        String::from_utf8_lossy(bytes).to_string()
    } else {
        "?".to_string()
    };
    let mut line: IMAGEHLP_LINE64 = std::mem::zeroed();
    line.SizeOfStruct = std::mem::size_of::<IMAGEHLP_LINE64>() as u32;
    let mut line_displacement = 0u32;
    let place = if SymGetLineFromAddr64(process, ip, &mut line_displacement, &mut line) != 0 {
        let file = CStr::from_ptr(line.FileName as *const i8).to_string_lossy().to_string();
        format!("{}:{}", file.rsplit(['\\', '/']).next().unwrap_or(&file), line.LineNumber)
    } else {
        "?".to_string()
    };
    (name, place)
}
