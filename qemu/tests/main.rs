use const_default::ConstDefault;

use qemu::qom_define_type;
use qemu::Object;
use qemu::ObjectClassMethods;
use qemu::ObjectImpl;

use qemu::qdev_define_type;
use qemu::qdev_prop;
use qemu::DeviceImpl;
use qemu::DeviceMethods;
use qemu::DeviceState;

use qemu::Result;

use std::cell::RefCell;

#[derive(Default, ConstDefault)]
struct TestConf {
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

qdev_define_type!(
    c"test-device",
    TestDevice,
    TestConf,
    RefCell<TestState>;
    @extends DeviceState;
    @properties [qdev_prop!(bool, c"foo", TestDevice, true, foo)]
);

impl TestDevice {
    #[allow(clippy::unused_self)]
    fn unparent(&self) {
        println!("unparent");
    }

    #[allow(clippy::unused_self)]
    fn realize(&self) -> Result<()> {
        println!("realize");
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn unrealize(&self) {
        println!("unrealize");
    }
}

impl ObjectImpl for TestDevice {
    const UNPARENT: Option<fn(&TestDevice)> = Some(TestDevice::unparent);
}

impl DeviceImpl for TestDevice {
    const REALIZE: Option<fn(&TestDevice) -> Result<()>> = Some(TestDevice::realize);
    const UNREALIZE: Option<fn(&TestDevice)> = Some(TestDevice::unrealize);
}

fn main() {
    drop(TestObject::new());

    let d = TestDevice::new();
    d.realize().unwrap();
    d.cold_reset();
    d.unparent();
}
