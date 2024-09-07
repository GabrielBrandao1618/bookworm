use std::{
    fmt::Debug,
    io::{BufReader, Read, Seek, SeekFrom, Write},
};

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{BookwormError, BookwormResult};

pub struct Pager<'a, S: Read + Write + Seek> {
    pub data_source: &'a mut S,
    page_size: usize,
    pages_count: usize,
}

impl<'a, S: Read + Write + Seek> Pager<'a, S> {
    pub fn new(page_size: usize, data_source: &'a mut S) -> Self {
        let data_source_len = data_source.seek(SeekFrom::End(0)).unwrap_or(0) as usize;
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
        let page_offset = self.page_size * page;
        let mut r = BufReader::new(&mut self.data_source);
        r.seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not read page data".to_string()))?;
        let mut buf = vec![0; self.page_size];
        self.data_source
            .read_exact(&mut buf)
            .map_err(|_| BookwormError::new("Could not read page".to_string()))?;
        Ok(buf)
    }
    pub fn write_raw_page(&mut self, page: usize, data: &[u8]) -> BookwormResult<()> {
        if page >= self.pages_count {
            return Err(BookwormError::new("Page doesn't exist".to_string()));
        }
        let page_offset = self.page_size * page;
        self.data_source
            .seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not write to page".to_string()))?;
        let remaining_space = self.page_size - data.len();
        self.data_source
            .write_all(&data)
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        self.data_source
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
        self.write_raw_page(page, &serialized)
            .map_err(|_| BookwormError::new("Could not write page".to_string()))?;
        Ok(())
    }
    pub fn get_raw_iterator(self) -> RawPagerIterator<'a, S> {
        self.into()
    }
    pub fn get_iterator<T: DeserializeOwned>(self) -> PagerIterator<'a, S, T> {
        self.into()
    }
    pub fn push<T: Serialize>(&mut self, data: &T) -> BookwormResult<()> {
        self.pages_count += 1;
        self.write_page(self.pages_count - 1, data)?;
        Ok(())
    }
    pub fn pop(&mut self) -> BookwormResult<()> {
        self.pages_count -= 1;
        let page_offset = self.pages_count * self.page_size;
        self.data_source
            .seek(SeekFrom::Start(page_offset as u64))
            .map_err(|_| BookwormError::new("Could not read page".to_owned()))?;
        let data = vec![0; self.page_size];
        self.data_source
            .write_all(&data)
            .map_err(|_| BookwormError::new("Could not remove page".to_owned()))?;
        Ok(())
    }
}

pub struct RawPagerIterator<'a, S: Read + Write + Seek> {
    data_source: &'a mut S,
    page_size: usize,
}

impl<'a, S: Read + Write + Seek> Into<RawPagerIterator<'a, S>> for Pager<'a, S> {
    fn into(self) -> RawPagerIterator<'a, S> {
        let _ = self.data_source.rewind();
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
        let _ = self.data_source.rewind();
        PagerIterator {
            page_size: self.page_size,
            data_source: self.data_source,
            _marker: Default::default(),
        }
    }
}
