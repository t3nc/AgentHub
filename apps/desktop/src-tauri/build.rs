fn main() {
    // 图标是**链接期资源**：tauri-build 会生成 `out/resource.rc`（内含
    // `32512 ICON "…/icons/icon.ico"`）并编译成 `out/resource.lib`，再由 rustc 链接进 exe。
    // 它默认不跟踪 icons/ 的变化，于是 `tauri icon` 换完图再 `tauri build`，
    // 链接进去的仍是旧资源 —— Windows 上表现为 exe 文件图标、任务栏按钮、窗口标题栏
    // 全是旧图（安装包反而是新的，因为 NSIS 是在打包阶段现读 icon.ico）。
    //
    // 声明 rerun-if-changed 之后，图标一改构建脚本就会重跑，`resource.lib` 随之重新编译。
    // icon.png 也要跟踪：它被 `generate_context!` 编译成窗口默认图标。
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
