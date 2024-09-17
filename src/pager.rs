use std::{
    cell::RefCell,
    fmt::Debug,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    rc::Rc,
};

use bincode::serialize;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{BookwormError, BookwormResult};

pub struct Pager<S: Read + Write + Seek> {
    pub data_source: Rc<RefCell<S>>,
    page_size: usize,
    pub pages_count: usize,
}

impl<S: Read + Write + Seek> Pager<S> {
    pub fn new(page_size: usize, data_source: Rc<RefCell<S>>) -> Self {
        let mut data_source_ref = data_source.borrow_mut();
        let data_source_len = data_source_ref.seek(SeekFrom::End(0)).unwrap_or(0) as usize;
        drop(data_source_ref);
        let last_page = data_source_len / page_size;
        Self {
            page_size,
            data_source,
            pages_count: last_page,
        }
    }
    pub fn get_page<T: DeserializeOwned + Debug>(&mut self, page: usize) -> BookwormResult<T> {
        let raw_page = self.get_raw_page(page)?;
        let parsed: T = bincode::deserialize(&raw_page)
            .map_err(|_| BookwormError::new("Could not parse data".to_string()))?;
        Ok(parsed)
    }
    pub fn get_raw_page(&mut self, page: usize) -> BookwormResult<Vec<u8>> {
        if page >= self.pages_count {
            return Err(BookwormError::new("Page doesn't exist".to_string()));
        }
        let mut data_source = self.data_source.borrow_mut();
        let page_offset = self.page_size * page;
        let mut r = BufReader::new(&mut *data_source);
        r.seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not read page data".to_string()))?;
        let mut buf = vec![0; self.page_size];
        data_source
            .read_exact(&mut buf)
            .map_err(|_| BookwormError::new("Could not read page".to_string()))?;
        Ok(buf)
    }
    pub fn write_raw_page(&mut self, page: usize, data: &[u8]) -> BookwormResult<()> {
        if page >= self.pages_count {
            return Err(BookwormError::new("Page doesn't exist".to_string()));
        }
        if data.len() > self.page_size {
            return Err(BookwormError::new(
                "Could not write data to page: data is bigger than page".to_string(),
            ));
        }
        let mut data_source = self.data_source.borrow_mut();
        let page_offset = self.page_size * page;
        data_source
            .seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not write to page".to_string()))?;
        let remaining_space = self.page_size - data.len();
        data_source
            .write_all(&data)
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        data_source
            .write_all(&vec![0; remaining_space])
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        Ok(())
    }
    pub fn write_page<T: Serialize>(&mut self, page: usize, data: &T) -> BookwormResult<()> {
        if page >= self.pages_count {
            return Err(BookwormError::new("Page doesn't exist".to_string()));
        }
        let serialized = bincode::serialize(data)
            .map_err(|_| BookwormError::new("Could not serialize data".to_string()))?;
        if serialized.len() > self.page_size {
            return Err(BookwormError::new(
                "Could not write data to page: data is bigger than page".to_string(),
            ));
        }
        self.write_raw_page(page, &serialized)
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        Ok(())
    }
    pub fn into_raw_iterator(self, starting_page: usize) -> RawPagerIterator<S> {
        let mut data_source = self.data_source.borrow_mut();
        let _ = data_source.seek(SeekFrom::Start((self.page_size * starting_page) as u64));
        drop(data_source);
        RawPagerIterator {
            page_size: self.page_size,
            data_source: self.data_source,
        }
    }
    pub fn into_iterator<T: DeserializeOwned>(self, starting_page: usize) -> PagerIterator<S, T> {
        let mut data_source = self.data_source.borrow_mut();
        let _ = data_source.seek(SeekFrom::Start((self.page_size * starting_page) as u64));
        drop(data_source);
        PagerIterator {
            page_size: self.page_size,
            data_source: self.data_source,
            _marker: Default::default(),
        }
    }
    /// Creates a iterator without dropping the pager
    pub fn iter<T: DeserializeOwned + Debug>(&mut self, starting_page: usize) -> PagerIter<S, T> {
        PagerIter {
            curr_pos: starting_page,
            pager: self,
            _marker: std::marker::PhantomData::default(),
        }
    }
    /// Creates a raw iterator without dropping the pager
    pub fn raw_iter(&mut self, starting_page: usize) -> RawPagerIter<S> {
        RawPagerIter {
            curr_pos: starting_page,
            pager: self,
        }
    }
    pub fn push<T: Serialize>(&mut self, data: &T) -> BookwormResult<()> {
        self.pages_count += 1;
        self.write_page(self.pages_count - 1, data)?;
        Ok(())
    }
    pub fn push_raw(&mut self, data: &[u8]) -> BookwormResult<()> {
        self.pages_count += 1;
        self.write_raw_page(self.pages_count - 1, data)?;
        Ok(())
    }
    pub fn pop(&mut self) -> BookwormResult<()> {
        self.pages_count -= 1;
        let page_offset = self.pages_count * self.page_size;
        let mut data_source = self.data_source.borrow_mut();
        data_source
            .seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not read page".to_owned()))?;
        let data = vec![0; self.page_size];
        data_source
            .write_all(&data)
            .map_err(|_| BookwormError::new("Could not remove page".to_owned()))?;
        Ok(())
    }
    pub fn clear(&mut self) {
        self.pages_count = 0;
    }
}

pub struct RawPagerIterator<S: Read + Write + Seek> {
    data_source: Rc<RefCell<S>>,
    page_size: usize,
}

impl<S: Read + Write + Seek> Iterator for RawPagerIterator<S> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = vec![0; self.page_size];
        let mut data_source = self.data_source.borrow_mut();
        match data_source.read_exact(&mut buf) {
            Ok(_) => Some(buf),
            Err(_) => None,
        }
    }
}

pub struct PagerIterator<S: Read + Write + Seek, T: DeserializeOwned> {
    data_source: Rc<RefCell<S>>,
    page_size: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<S, T> Iterator for PagerIterator<S, T>
where
    S: Read + Write + Seek,
    T: DeserializeOwned,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = vec![0; self.page_size];
        let mut data_source = self.data_source.borrow_mut();
        if let Ok(_) = data_source.read_exact(&mut buf) {
            if let Ok(parsed) = bincode::deserialize(&buf) {
                return Some(parsed);
            }
        }
        None
    }
}

pub struct PagerIter<'a, S: Read + Write + Seek, T: DeserializeOwned + Debug> {
    curr_pos: usize,
    pager: &'a mut Pager<S>,
    _marker: std::marker::PhantomData<T>,
}
impl<'a, S, T: DeserializeOwned + Debug> Iterator for PagerIter<'a, S, T>
where
    S: Read + Write + Seek,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(page) = self.pager.get_page(self.curr_pos) {
            self.curr_pos += 1;
            Some(page)
        } else {
            None
        }
    }
}
pub struct RawPagerIter<'a, S: Read + Write + Seek> {
    curr_pos: usize,
    pager: &'a mut Pager<S>,
}

impl<'a, S> Iterator for RawPagerIter<'a, S>
where
    S: Read + Write + Seek,
{
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(page) = self.pager.get_raw_page(self.curr_pos) {
            self.curr_pos += 1;
            Some(page)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        count: u8,
        checked: bool,
    }
    impl TestData {
        pub fn new(count: u8, checked: bool) -> Self {
            Self { count, checked }
        }
    }
    #[test]
    fn test_iter() {
        let data_source = Rc::new(RefCell::new(Cursor::new(Vec::new())));
        let mut pager = Pager::new(128, data_source);
        let test_data_1 = TestData::new(10, true);
        let test_data_2 = TestData::new(12, false);
        pager.push(&test_data_1).unwrap();
        pager.push(&test_data_2).unwrap();
        let mut iter = pager.iter::<TestData>(0);
        assert_eq!(iter.next().unwrap(), test_data_1);
        assert_eq!(iter.next().unwrap(), test_data_2);
    }
    #[test]
    fn test_raw_iter() {
        let data_source = Rc::new(RefCell::new(Cursor::new(Vec::new())));
        let mut pager = Pager::new(128, data_source);
        pager.push_raw(b"apple").unwrap();
        pager.push_raw(b"grape").unwrap();
        let mut iter = pager.raw_iter(0);
        let first = &iter.next().unwrap();
        let second = &iter.next().unwrap();
        let parsed_first = String::from_utf8_lossy(first);
        let parsed_second = String::from_utf8_lossy(second);
        assert!(parsed_first.contains("apple"));
        assert!(parsed_second.contains("grape"));
    }
}
