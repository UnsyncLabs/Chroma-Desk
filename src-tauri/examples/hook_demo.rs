//! Installs the Omarchy hooks for real (`omarchy hook install`), prints their status and removes them again.
//! `cargo run --example hook_demo [install|remove]`
use chroma_desk_lib::themes::{hook, sources::Env};

fn main() {
    let env = Env::from_env();
    match std::env::args().nth(1).as_deref() {
        Some("install") => {
            hook::install(&env).expect("theme-set hook");
            for k in hook::EVENTS {
                hook::install_event(&env, k).expect("event hook");
            }
        }
        Some("remove") => {
            hook::remove(&env).expect("remove theme-set hook");
            for k in hook::EVENTS {
                hook::remove_event(&env, k).expect("remove event hook");
            }
        }
        _ => {}
    }
    println!("palette hook: {:?}", hook::status(&env));
    for e in hook::event_status(&env) {
        println!("{} installed={} path={}", e.kind, e.installed, e.path);
    }
}
