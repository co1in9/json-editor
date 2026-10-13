fn main() {
    println!("cargo:rerun-if-changed=assets/icon/windows.rc");
    println!("cargo:rerun-if-changed=assets/icon/json-editor.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/icon/windows.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed the Windows application icon");
    }
}
