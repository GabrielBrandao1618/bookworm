use std::io::{BufReader, Read, Seek, SeekFrom, Write};

use error::{BookwormError, BookwormResult};
use serde::{de::DeserializeOwned, ser::Serialize};

pub mod error;

pub struct Pager<'a, S: Read + Write + Seek> {
    data_source: &'a mut S,
    page_size: usize,
}
impl<'a, S: Read + Write + Seek> Pager<'a, S> {
    pub fn new(page_size: usize, data_source: &'a mut S) -> Self {
        Self {
            page_size,
            data_source,
        }
    }
    pub fn get_page<T: DeserializeOwned>(&mut self, page: usize) -> BookwormResult<T> {
        let raw_page = self.get_raw_page(page)?;
        let parsed: T = bincode::deserialize(&raw_page)
            .map_err(|_| BookwormError::new("Could not parse data".to_string()))?;
        Ok(parsed)
    }
    pub fn get_raw_page(&mut self, page: usize) -> BookwormResult<Vec<u8>> {
        let page_offset = self.page_size * page;
        let mut r = BufReader::new(&mut self.data_source);
        r.seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not read page data".to_string()))?;
        let mut buf = vec![0; self.page_size];
        r.read_exact(&mut buf)
            .map_err(|_| BookwormError::new("Could not read page".to_string()))?;
        Ok(buf)
    }
    pub fn write_page<T: Serialize>(&mut self, page: usize, data: &T) -> BookwormResult<()> {
        let page_offset = self.page_size * page;
        self.data_source
            .seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not write to page".to_string()))?;
        let serialized = bincode::serialize(data)
            .map_err(|_| BookwormError::new("Could not serialize data".to_string()))?;
        let serialized_len = serialized.len();
        let remaining_space = self.page_size - serialized_len;
        self.data_source
            .write_all(&serialized)
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        self.data_source
            .write_all(&vec![0; remaining_space])
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        Ok(())
    }
    pub fn get_raw_iterator(self) -> RawPagerIterator<'a, S> {
        let _ = self.data_source.seek(SeekFrom::Start(0));
        self.into()
    }
    pub fn get_iterator<T: DeserializeOwned>(self) -> PagerIterator<'a, S, T> {
        let _ = self.data_source.seek(SeekFrom::Start(0));
        self.into()
    }
}

pub struct RawPagerIterator<'a, S: Read + Write + Seek> {
    data_source: &'a mut S,
    page_size: usize,
}

impl<'a, S: Read + Write + Seek> Into<RawPagerIterator<'a, S>> for Pager<'a, S> {
    fn into(self) -> RawPagerIterator<'a, S> {
        let _ = self.data_source.seek(SeekFrom::Start(0));
        RawPagerIterator {
            data_source: self.data_source,
            page_size: self.page_size,
        }
    }
}

impl<S: Read + Write + Seek> Iterator for RawPagerIterator<'_, S> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = vec![0; self.page_size];
        match self.data_source.read_exact(&mut buf) {
            Ok(_) => Some(buf),
            Err(_) => None,
        }
    }
}

pub struct PagerIterator<'a, S: Read + Write + Seek, T: DeserializeOwned> {
    data_source: &'a mut S,
    page_size: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<S, T> Iterator for PagerIterator<'_, S, T>
where
    S: Read + Write + Seek,
    T: DeserializeOwned,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = vec![0; self.page_size];
        if let Ok(_) = self.data_source.read_exact(&mut buf) {
            if let Ok(parsed) = bincode::deserialize(&buf) {
                return Some(parsed);
            }
        }
        None
    }
}

impl<'a, S: Read + Write + Seek, T: DeserializeOwned> Into<PagerIterator<'a, S, T>>
    for Pager<'a, S>
{
    fn into(self) -> PagerIterator<'a, S, T> {
        let _ = self.data_source.seek(SeekFrom::Start(0));
        PagerIterator {
            page_size: self.page_size,
            data_source: self.data_source,
            _marker: Default::default(),
        }
    }
}

#[cfg(test)]
pub mod tests {
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
        let mut pager = Pager::new(1024, &mut data_source);
        let test_data1 = TestData::new(10, true);
        let test_data2 = TestData::new(15, false);
        let test_data3 = TestData::new(20, true);
        pager.write_page(0, &test_data1).unwrap();
        pager.write_page(1, &test_data2).unwrap();
        pager.write_page(2, &test_data3).unwrap();

        assert_eq!(pager.get_page::<TestData>(0).unwrap(), test_data1);
        assert_eq!(pager.get_page::<TestData>(1).unwrap(), test_data2);
        assert_eq!(pager.get_page::<TestData>(2).unwrap(), test_data3);
    }

    #[test]
    fn test_iter_pages() {
        let mut data_source = Cursor::new(Vec::new());
        let mut pager = Pager::new(1024, &mut data_source);
        pager.write_page(0, &TestData::new(10, true)).unwrap();
        pager.write_page(1, &TestData::new(14, false)).unwrap();
        pager.write_page(2, &TestData::new(17, true)).unwrap();
        pager.write_page(3, &TestData::new(6, false)).unwrap();

        let mut iterator = pager.get_iterator::<TestData>();
        assert_eq!(iterator.next().unwrap(), TestData::new(10, true));
        assert_eq!(iterator.next().unwrap(), TestData::new(14, false));
        assert_eq!(iterator.next().unwrap(), TestData::new(17, true));
        assert_eq!(iterator.next().unwrap(), TestData::new(6, false));
        assert_eq!(iterator.next(), None);
    }
}
