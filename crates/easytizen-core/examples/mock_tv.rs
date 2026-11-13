//! Protocol-level Samsung TV emulator for developing EasyTizen without a television.
//!
//!   cargo run -p easytizen-core --example mock_tv --features mock-tv -- [bind-ip] [port] [duid]
//!
//! Then add the printed address in EasyTizen ("Add by IP"). The emulator accepts packages
//! whose distributor certificate names its DUID, exactly like a TV would — but it is not a TV.

use easytizen_core::sdb::mock::MockTv;
use std::net::Ipv4Addr;

fn main() {
    let mut args = std::env::args().skip(1);
    let ip: Ipv4Addr = args
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(Ipv4Addr::UNSPECIFIED);
    let port: u16 = args.next().and_then(|a| a.parse().ok()).unwrap_or(26101);
    let duid = args
        .next()
        .unwrap_or_else(|| "EMULATEDTV0001DUID".to_string());
    let tv = MockTv::start(ip, port, &duid).expect("cannot listen on that address");
    println!(
        "Emulated TV listening on {ip}:{} (DUID {duid}). Press Ctrl+C to stop.",
        tv.port
    );
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let state = tv.state.lock().unwrap();
        if !state.installed.is_empty() {
            println!(
                "installed: {:?}  launched: {:?}",
                state.installed, state.launched
            );
        }
    }
}
