// 릴리스 빌드에서는 콘솔 창을 띄우지 않는다.
// (디버그 빌드에서는 stdout 로그를 봐야 하므로 콘솔을 남긴다.)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    chewie_app_lib::run()
}
