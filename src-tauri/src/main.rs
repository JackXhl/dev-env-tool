#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--scan") {
        let items = dev_env_tool_lib::scan_cli();
        if args.iter().any(|a| a == "--json") {
            println!(
                "{}",
                serde_json::to_string_pretty(&items).expect("serialize scan")
            );
            return;
        }
        for it in items {
            println!(
                "{}\t{}\t{}\texe={}\thome={}\tdata={}\tport={}\tsvc={}",
                it.id,
                it.status,
                it.summary,
                it.executable,
                it.home_dir,
                it.data_dir,
                it.port,
                it.service_name
            );
        }
        return;
    }
    dev_env_tool_lib::run();
}
