use std::io::Cursor;

use serde::{Deserialize, Serialize};

use super::*;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TestData {
    pub count: u8,
    pub signed: bool,
}
impl TestData {
    pub fn new(count: u8, signed: bool) -> Self {
        Self { count, signed }
    }
}

#[test]
fn test_read_write() {
    let mut data_source = Cursor::new(Vec::new());
    let mut swap = Cursor::new(Vec::new());
    let mut bookworm = Bookworm::new(1024, &mut data_source, &mut swap);
    let test_data1 = TestData::new(10, true);
    let test_data2 = TestData::new(15, false);
    let test_data3 = TestData::new(20, true);
    bookworm.push(&test_data1).unwrap();
    bookworm.push(&test_data2).unwrap();
    bookworm.push(&test_data3).unwrap();

    assert_eq!(bookworm.get_page::<TestData>(0).unwrap(), test_data1);
    assert_eq!(bookworm.get_page::<TestData>(1).unwrap(), test_data2);
    assert_eq!(bookworm.get_page::<TestData>(2).unwrap(), test_data3);
}

#[test]
fn test_iter_pages() {
    let mut data_source = Cursor::new(Vec::new());
    let mut swap = Cursor::new(Vec::new());
    let mut bookworm = Bookworm::new(1024, &mut data_source, &mut swap);
    bookworm.push(&TestData::new(10, true)).unwrap();
    bookworm.push(&TestData::new(14, false)).unwrap();
    bookworm.push(&TestData::new(17, true)).unwrap();
    bookworm.push(&TestData::new(6, false)).unwrap();

    let mut iterator = bookworm.get_iterator::<TestData>();
    assert_eq!(iterator.next().unwrap(), TestData::new(10, true));
    assert_eq!(iterator.next().unwrap(), TestData::new(14, false));
    assert_eq!(iterator.next().unwrap(), TestData::new(17, true));
    assert_eq!(iterator.next().unwrap(), TestData::new(6, false));
    assert_eq!(iterator.next(), None);
}
#[test]
fn test_push() {
    let mut data_source = Cursor::new(Vec::new());
    let mut swap = Cursor::new(Vec::new());
    let mut bookworm = Bookworm::new(1024, &mut data_source, &mut swap);

    bookworm.push(&TestData::new(10, true)).unwrap();
    bookworm.push(&TestData::new(12, false)).unwrap();
    bookworm.push(&TestData::new(6, true)).unwrap();

    let mut iterator = bookworm.get_iterator::<TestData>();
    assert_eq!(iterator.next().unwrap(), TestData::new(10, true));
    assert_eq!(iterator.next().unwrap(), TestData::new(12, false));
    assert_eq!(iterator.next().unwrap(), TestData::new(6, true));

    drop(iterator);
    let mut pager = Bookworm::new(1024, &mut data_source, &mut swap);
    pager.push(&TestData::new(18, false)).unwrap();
    let mut iterator = pager.get_iterator::<TestData>();
    assert_eq!(iterator.next().unwrap(), TestData::new(10, true));
    assert_eq!(iterator.next().unwrap(), TestData::new(12, false));
    assert_eq!(iterator.next().unwrap(), TestData::new(6, true));
    assert_eq!(iterator.next().unwrap(), TestData::new(18, false));
}
#[test]
fn test_remove_page() {
    let mut data_source = Cursor::new(Vec::new());
    let mut swap = Cursor::new(Vec::new());
    let mut pager = Bookworm::new(32, &mut data_source, &mut swap);
    let test_data = TestData::new(10, true);
    pager.push(&test_data).unwrap();
    pager.get_page::<TestData>(0).unwrap();
    pager.pop().unwrap();
    pager.get_page::<TestData>(0).unwrap_err();
}
