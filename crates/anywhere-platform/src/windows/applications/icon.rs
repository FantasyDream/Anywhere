use super::super::shell::{self, Apartment};
use crate::RawIconImage;
use anyhow::Result;
use std::{mem::size_of, path::Path, ptr::null_mut};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::SIZE,
        UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY},
    },
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
};

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}

pub(in crate::windows) fn icon(target: &str, size: u32) -> Result<Option<RawIconImage>> {
    if !Path::new(target).is_absolute() && super::packaged_target(target).is_none() {
        return Ok(None);
    }
    let _apartment = Apartment::open()?;
    let target = shell::wide(target.as_ref())?;
    let Ok(factory) = (unsafe {
        SHCreateItemFromParsingName::<_, _, IShellItemImageFactory>(PCWSTR(target.as_ptr()), None)
    }) else {
        return Ok(None);
    };
    let size = size.clamp(16, 256) as i32;
    let Ok(bitmap) = (unsafe { factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY) })
    else {
        return Ok(None);
    };
    Ok(read_bitmap(Bitmap(bitmap.0)))
}

fn read_bitmap(bitmap: Bitmap) -> Option<RawIconImage> {
    let mut info = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            bitmap.0,
            size_of::<BITMAP>() as i32,
            std::ptr::from_mut(&mut info).cast(),
        )
    };
    if read == 0 || !(1..=256).contains(&info.bmWidth) || !(1..=256).contains(&info.bmHeight) {
        return None;
    }
    let width = info.bmWidth as u32;
    let height = info.bmHeight as u32;
    let mut dib = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: info.bmWidth,
            biHeight: -info.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0; (width * height * 4) as usize];
    let dc = unsafe { CreateCompatibleDC(null_mut()) };
    if dc.is_null() {
        return None;
    }
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap.0,
            0,
            height,
            bgra.as_mut_ptr().cast(),
            &mut dib,
            DIB_RGB_COLORS,
        )
    };
    unsafe { DeleteDC(dc) };
    (lines == info.bmHeight && bgra.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0)).then_some(
        RawIconImage {
            width,
            height,
            bgra,
        },
    )
}
