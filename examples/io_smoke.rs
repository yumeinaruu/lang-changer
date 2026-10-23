//! Manual check for inject + input_source. Focus a text field, run, wait 3 seconds.
use lang_changer::{inject, input_source, layout_map::Direction};
use std::{thread, time::Duration};

fn main() {
    println!("current layout: {:?}", input_source::current_id());
    thread::sleep(Duration::from_secs(3));
    inject::replace(0, "привет");
    input_source::switch_to(Direction::EnToRu);
    thread::sleep(Duration::from_millis(300));
    println!("layout after switch: {:?}", input_source::current_id());
}
