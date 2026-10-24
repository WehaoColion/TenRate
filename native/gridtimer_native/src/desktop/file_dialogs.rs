// v2.22.35 - Native, owned file dialogs for portable backups and note exports.

fn desktop_transfer_file_dialog(
    save: bool,
    title: &str,
    default_name: &str,
    extension: &str,
    label: &str,
) -> io::Result<Option<PathBuf>> {
    #[cfg(target_os = "windows")]
    {
        transfer_native_dialog::choose(save, title, default_name, extension, label)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (save, title, default_name, extension, label);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "此平台没有原生文件选择器",
        ))
    }
}

/// Select several files through the same owner-bound native Windows dialog.
fn desktop_transfer_multiple_images_dialog() -> io::Result<Option<Vec<PathBuf>>> {
    #[cfg(target_os = "windows")]
    {
        transfer_native_dialog::choose_many_images()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(io::Error::new(io::ErrorKind::Unsupported, "此平台没有原生文件选择器"))
    }
}

#[cfg(target_os = "windows")]
mod transfer_native_dialog {
    use super::*;
    use std::os::windows::ffi::OsStringExt;

    #[repr(C)]
    struct OpenFileNameW {
        size: u32,
        owner: *mut c_void,
        instance: *mut c_void,
        filter: *const u16,
        custom_filter: *mut u16,
        max_custom_filter: u32,
        filter_index: u32,
        file: *mut u16,
        max_file: u32,
        file_title: *mut u16,
        max_file_title: u32,
        initial_directory: *const u16,
        title: *const u16,
        flags: u32,
        file_offset: u16,
        extension_offset: u16,
        default_extension: *const u16,
        custom_data: isize,
        hook: usize,
        template_name: *const u16,
        reserved: *mut c_void,
        reserved_flags: u32,
        extended_flags: u32,
    }

    #[link(name = "comdlg32")]
    extern "system" {
        fn GetOpenFileNameW(value: *mut OpenFileNameW) -> i32;
        fn GetSaveFileNameW(value: *mut OpenFileNameW) -> i32;
        fn CommDlgExtendedError() -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetActiveWindow() -> *mut c_void;
    }

    pub(super) fn choose(
        save: bool,
        title: &str,
        default_name: &str,
        extension: &str,
        label: &str,
    ) -> io::Result<Option<PathBuf>> {
        let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let title = wide(title);
        let extension_wide = wide(extension);
        let filter = wide(&format!("{label}\0*.{extension}\0所有文件\0*.*\0"));
        let mut buffer = vec![0_u16; 32_768];
        let name = default_name.encode_utf16().collect::<Vec<_>>();
        if name.len() >= buffer.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "文件名过长"));
        }
        buffer[..name.len()].copy_from_slice(&name);
        // Zero is valid for all optional pointers, flags, and the unused hook.
        let mut value: OpenFileNameW = unsafe { std::mem::zeroed() };
        value.size = std::mem::size_of::<OpenFileNameW>() as u32;
        value.owner = unsafe { GetActiveWindow() };
        value.filter = filter.as_ptr();
        value.filter_index = 1;
        value.file = buffer.as_mut_ptr();
        value.max_file = buffer.len() as u32;
        value.title = title.as_ptr();
        value.default_extension = extension_wide.as_ptr();
        // Explorer, no working-directory changes, existing destination folder.
        value.flags = 0x0008_0000 | 0x0000_0008 | 0x0000_0800 | 0x0000_0004;
        value.flags |= if save { 0x0000_0002 } else { 0x0000_1000 };
        let accepted = unsafe {
            if save {
                GetSaveFileNameW(&mut value)
            } else {
                GetOpenFileNameW(&mut value)
            }
        };
        if accepted == 0 {
            let code = unsafe { CommDlgExtendedError() };
            return if code == 0 {
                Ok(None)
            } else {
                Err(io::Error::other(format!("文件选择器错误：0x{code:08x}")))
            };
        }
        let len = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &buffer[..len],
        ))))
    }
    fn selection_paths(buffer: &[u16]) -> io::Result<Vec<PathBuf>> {
        let parts = buffer.split(|ch| *ch == 0)
            .take_while(|part| !part.is_empty())
            .map(|part| PathBuf::from(std::ffi::OsString::from_wide(part)))
            .collect::<Vec<_>>();
        let Some(first) = parts.first() else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "文件选择结果为空"));
        };
        if parts.len() == 1 { return Ok(vec![first.clone()]); }
        // Explorer multi-select returns folder + relative file names.
        if parts[1..].iter().any(|item| item.is_absolute() || item.components().count() != 1) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "多选文件名不合法"));
        }
        Ok(parts[1..].iter().map(|part| first.join(part)).collect())
    }

    pub(super) fn choose_many_images() -> io::Result<Option<Vec<PathBuf>>> {
        let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let title = wide("导入图片（可多选）");
        let filter = wide("图片\0*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp\0所有文件\0*.*\0");
        let mut buffer = vec![0_u16; 65_536];
        let mut value: OpenFileNameW = unsafe { std::mem::zeroed() };
        value.size = std::mem::size_of::<OpenFileNameW>() as u32;
        value.owner = unsafe { GetActiveWindow() };
        value.filter = filter.as_ptr();
        value.filter_index = 1;
        value.file = buffer.as_mut_ptr();
        value.max_file = buffer.len() as u32;
        value.title = title.as_ptr();
        // Explorer + multi-select + file-must-exist + no CWD changes.
        value.flags = 0x0008_0000 | 0x0000_0200 | 0x0000_1000 | 0x0000_0800 | 0x0000_0008 | 0x0000_0004;
        if unsafe { GetOpenFileNameW(&mut value) } == 0 {
            let code = unsafe { CommDlgExtendedError() };
            return if code == 0 { Ok(None) }
                else { Err(io::Error::other(format!("文件选择器错误：0x{code:08x}"))) };
        }
        let paths = selection_paths(&buffer)?;
        if paths.len() > 12 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "一次最多选取 12 张图片"));
        }
        Ok(Some(paths))
    }

    #[cfg(test)]
    mod many_image_selection_tests {
        use super::*;
        #[test]
        fn parses_single_and_folder_based_multi_selection() {
            let buf = |s: &str| s.encode_utf16().collect::<Vec<_>>();
            let single = buf("C:\\images\\first.png\0\0");
            assert_eq!(selection_paths(&single).unwrap(), vec![PathBuf::from("C:\\images\\first.png")]);
            let multi = buf("C:\\images\0first.png\0second.jpg\0\0");
            assert_eq!(selection_paths(&multi).unwrap(), vec![PathBuf::from("C:\\images\\first.png"),PathBuf::from("C:\\images\\second.jpg")]);
            assert!(selection_paths(&buf("C:\\images\0..\\escape.png\0\0")).is_err());
        }
    }

}
