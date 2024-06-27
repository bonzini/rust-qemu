use const_default::ConstDefault;

use qemu::qom_define_type;
use qemu::Object;
use qemu::ObjectClassMethods;
use qemu::ObjectImpl;

#[derive(Default, ConstDefault)]
struct TestConf {
    #[allow(dead_code)]
    foo: bool,
}

#[derive(Default)]
struct TestState {
    #[allow(dead_code)]
    bar: i32,
}

qom_define_type!(
    c"test-object",
    TestObject,
    TestConf,
    ();
    @extends Object
);

impl ObjectImpl for TestObject {}

fn main() {
    drop(TestObject::new());
}
