extern crate embed_resource;

fn main() {
    slint_build::compile("ui/appwindow.slint").unwrap();
    embed_resource::compile("icon.rc", embed_resource::NONE);
}