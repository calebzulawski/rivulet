#![allow(unused)]

fn view_is_object_safe() -> Option<&'static mut dyn rivulet::View<Item = (), Error = ()>> {
    None
}

fn viewmut_is_object_safe() -> Option<&'static mut dyn rivulet::ViewMut<Item = (), Error = ()>> {
    None
}
