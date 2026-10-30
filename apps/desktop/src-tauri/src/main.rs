// В выпускной сборке под Windows не открывать консольное окно.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    plainrussian_desktop_lib::run()
}
