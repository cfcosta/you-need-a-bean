// Find the nesting depth a given stack size survives, by bisection in a
// child process (a stack overflow aborts, so it cannot be caught in-process).
fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("try") => {
            let stack: usize = args.next().unwrap().parse().unwrap();
            let depth: usize = args.next().unwrap().parse().unwrap();
            std::thread::Builder::new()
                .stack_size(stack)
                .spawn(move || {
                    let expr =
                        format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
                    let out = bean_conformance::dump(&format!(
                        "2026-01-01 price A {expr} USD\n"
                    ));
                    std::hint::black_box(out);
                })
                .unwrap()
                .join()
                .unwrap();
        }
        _ => {
            let exe = std::env::current_exe().unwrap();
            for stack in [1 << 21, 1 << 23] {
                let (mut lo, mut hi) = (1usize, 100_000usize);
                while lo + 1 < hi {
                    let mid = (lo + hi) / 2;
                    let ok = std::process::Command::new(&exe)
                        .args(["try", &stack.to_string(), &mid.to_string()])
                        .stderr(std::process::Stdio::null())
                        .status()
                        .unwrap()
                        .success();
                    if ok { lo = mid } else { hi = mid }
                }
                println!("stack {:>8} bytes: max depth {lo}", stack);
            }
        }
    }
}
