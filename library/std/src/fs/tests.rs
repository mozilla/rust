use crate::io::prelude::*;

use crate::fs::{self, File, OpenOptions};
use crate::io::{ErrorKind, SeekFrom};
use crate::path::Path;
use crate::str;
use crate::sys_common::io::test::{tmpdir, TempDir};
use crate::thread;

use rand::{rngs::StdRng, RngCore, SeedableRng};

#[cfg(unix)]
use crate::os::unix::fs::symlink as symlink_dir;
#[cfg(unix)]
use crate::os::unix::fs::symlink as symlink_file;
#[cfg(unix)]
use crate::os::unix::fs::symlink as symlink_junction;
#[cfg(windows)]
use crate::os::windows::fs::{symlink_dir, symlink_file};
#[cfg(windows)]
use crate::sys::fs::symlink_junction;

macro_rules! check {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(e) => panic!("{} failed with: {}", stringify!($e), e),
        }
    };
}

#[cfg(windows)]
macro_rules! error {
    ($e:expr, $s:expr) => {
        match $e {
            Ok(_) => panic!("Unexpected success. Should've been: {:?}", $s),
            Err(ref err) => assert!(
                err.raw_os_error() == Some($s),
                format!("`{}` did not have a code of `{}`", err, $s)
            ),
        }
    };
}

#[cfg(unix)]
macro_rules! error {
    ($e:expr, $s:expr) => {
        error_contains!($e, $s)
    };
}

macro_rules! error_contains {
    ($e:expr, $s:expr) => {
        match $e {
            Ok(_) => panic!("Unexpected success. Should've been: {:?}", $s),
            Err(ref err) => {
                assert!(err.to_string().contains($s), format!("`{}` did not contain `{}`", err, $s))
            }
        }
    };
}

// Several test fail on windows if the user does not have permission to
// create symlinks (the `SeCreateSymbolicLinkPrivilege`). Instead of
// disabling these test on Windows, use this function to test whether we
// have permission, and return otherwise. This way, we still don't run these
// tests most of the time, but at least we do if the user has the right
// permissions.
pub fn got_symlink_permission(tmpdir: &TempDir) -> bool {
    if cfg!(unix) {
        return true;
    }
    let link = tmpdir.join("some_hopefully_unique_link_name");

    match symlink_file(r"nonexisting_target", link) {
        // ERROR_PRIVILEGE_NOT_HELD = 1314
        Err(ref err) if err.raw_os_error() == Some(1314) => false,
        Ok(_) | Err(_) => true,
    }
}

#[test]
fn file_test_io_smoke_test() {
    let message = "it's alright. have a good time";
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test.txt");
    {
        let mut write_stream = check!(File::create(filename));
        check!(write_stream.write(message.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open(filename));
        let mut read_buf = [0; 1028];
        let read_str = match check!(read_stream.read(&mut read_buf)) {
            0 => panic!("shouldn't happen"),
            n => str::from_utf8(&read_buf[..n]).unwrap().to_string(),
        };
        assert_eq!(read_str, message);
    }
    check!(fs::remove_file(filename));
}

#[test]
fn invalid_path_raises() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_that_does_not_exist.txt");
    let result = File::open(filename);

    #[cfg(all(unix, not(target_os = "vxworks")))]
    error!(result, "No such file or directory");
    #[cfg(target_os = "vxworks")]
    error!(result, "no such file or directory");
    #[cfg(windows)]
    error!(result, 2); // ERROR_FILE_NOT_FOUND
}

#[test]
fn file_test_iounlinking_invalid_path_should_raise_condition() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_another_file_that_does_not_exist.txt");

    let result = fs::remove_file(filename);

    #[cfg(all(unix, not(target_os = "vxworks")))]
    error!(result, "No such file or directory");
    #[cfg(target_os = "vxworks")]
    error!(result, "no such file or directory");
    #[cfg(windows)]
    error!(result, 2); // ERROR_FILE_NOT_FOUND
}

#[test]
fn file_test_io_non_positional_read() {
    let message: &str = "ten-four";
    let mut read_mem = [0; 8];
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_positional.txt");
    {
        let mut rw_stream = check!(File::create(filename));
        check!(rw_stream.write(message.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open(filename));
        {
            let read_buf = &mut read_mem[0..4];
            check!(read_stream.read(read_buf));
        }
        {
            let read_buf = &mut read_mem[4..8];
            check!(read_stream.read(read_buf));
        }
    }
    check!(fs::remove_file(filename));
    let read_str = str::from_utf8(&read_mem).unwrap();
    assert_eq!(read_str, message);
}

#[test]
fn file_test_io_seek_and_tell_smoke_test() {
    let message = "ten-four";
    let mut read_mem = [0; 4];
    let set_cursor = 4 as u64;
    let tell_pos_pre_read;
    let tell_pos_post_read;
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_seeking.txt");
    {
        let mut rw_stream = check!(File::create(filename));
        check!(rw_stream.write(message.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open(filename));
        check!(read_stream.seek(SeekFrom::Start(set_cursor)));
        tell_pos_pre_read = check!(read_stream.seek(SeekFrom::Current(0)));
        check!(read_stream.read(&mut read_mem));
        tell_pos_post_read = check!(read_stream.seek(SeekFrom::Current(0)));
    }
    check!(fs::remove_file(filename));
    let read_str = str::from_utf8(&read_mem).unwrap();
    assert_eq!(read_str, &message[4..8]);
    assert_eq!(tell_pos_pre_read, set_cursor);
    assert_eq!(tell_pos_post_read, message.len() as u64);
}

#[test]
fn file_test_io_seek_and_write() {
    let initial_msg = "food-is-yummy";
    let overwrite_msg = "-the-bar!!";
    let final_msg = "foo-the-bar!!";
    let seek_idx = 3;
    let mut read_mem = [0; 13];
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_seek_and_write.txt");
    {
        let mut rw_stream = check!(File::create(filename));
        check!(rw_stream.write(initial_msg.as_bytes()));
        check!(rw_stream.seek(SeekFrom::Start(seek_idx)));
        check!(rw_stream.write(overwrite_msg.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open(filename));
        check!(read_stream.read(&mut read_mem));
    }
    check!(fs::remove_file(filename));
    let read_str = str::from_utf8(&read_mem).unwrap();
    assert!(read_str == final_msg);
}

#[test]
fn file_test_io_seek_shakedown() {
    //                   01234567890123
    let initial_msg = "qwer-asdf-zxcv";
    let chunk_one: &str = "qwer";
    let chunk_two: &str = "asdf";
    let chunk_three: &str = "zxcv";
    let mut read_mem = [0; 4];
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_seek_shakedown.txt");
    {
        let mut rw_stream = check!(File::create(filename));
        check!(rw_stream.write(initial_msg.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open(filename));

        check!(read_stream.seek(SeekFrom::End(-4)));
        check!(read_stream.read(&mut read_mem));
        assert_eq!(str::from_utf8(&read_mem).unwrap(), chunk_three);

        check!(read_stream.seek(SeekFrom::Current(-9)));
        check!(read_stream.read(&mut read_mem));
        assert_eq!(str::from_utf8(&read_mem).unwrap(), chunk_two);

        check!(read_stream.seek(SeekFrom::Start(0)));
        check!(read_stream.read(&mut read_mem));
        assert_eq!(str::from_utf8(&read_mem).unwrap(), chunk_one);
    }
    check!(fs::remove_file(filename));
}

#[test]
fn file_test_io_eof() {
    let tmpdir = tmpdir();
    let filename = tmpdir.join("file_rt_io_file_test_eof.txt");
    let mut buf = [0; 256];
    {
        let oo = OpenOptions::new().create_new(true).write(true).read(true).clone();
        let mut rw = check!(oo.open(&filename));
        assert_eq!(check!(rw.read(&mut buf)), 0);
        assert_eq!(check!(rw.read(&mut buf)), 0);
    }
    check!(fs::remove_file(&filename));
}

#[test]
#[cfg(unix)]
fn file_test_io_read_write_at() {
    use crate::os::unix::fs::FileExt;

    let tmpdir = tmpdir();
    let filename = tmpdir.join("file_rt_io_file_test_read_write_at.txt");
    let mut buf = [0; 256];
    let write1 = "asdf";
    let write2 = "qwer-";
    let write3 = "-zxcv";
    let content = "qwer-asdf-zxcv";
    {
        let oo = OpenOptions::new().create_new(true).write(true).read(true).clone();
        let mut rw = check!(oo.open(&filename));
        assert_eq!(check!(rw.write_at(write1.as_bytes(), 5)), write1.len());
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 0);
        assert_eq!(check!(rw.read_at(&mut buf, 5)), write1.len());
        assert_eq!(str::from_utf8(&buf[..write1.len()]), Ok(write1));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 0);
        assert_eq!(check!(rw.read_at(&mut buf[..write2.len()], 0)), write2.len());
        assert_eq!(str::from_utf8(&buf[..write2.len()]), Ok("\0\0\0\0\0"));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 0);
        assert_eq!(check!(rw.write(write2.as_bytes())), write2.len());
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 5);
        assert_eq!(check!(rw.read(&mut buf)), write1.len());
        assert_eq!(str::from_utf8(&buf[..write1.len()]), Ok(write1));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 9);
        assert_eq!(check!(rw.read_at(&mut buf[..write2.len()], 0)), write2.len());
        assert_eq!(str::from_utf8(&buf[..write2.len()]), Ok(write2));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 9);
        assert_eq!(check!(rw.write_at(write3.as_bytes(), 9)), write3.len());
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 9);
    }
    {
        let mut read = check!(File::open(&filename));
        assert_eq!(check!(read.read_at(&mut buf, 0)), content.len());
        assert_eq!(str::from_utf8(&buf[..content.len()]), Ok(content));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 0);
        assert_eq!(check!(read.seek(SeekFrom::End(-5))), 9);
        assert_eq!(check!(read.read_at(&mut buf, 0)), content.len());
        assert_eq!(str::from_utf8(&buf[..content.len()]), Ok(content));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 9);
        assert_eq!(check!(read.read(&mut buf)), write3.len());
        assert_eq!(str::from_utf8(&buf[..write3.len()]), Ok(write3));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
        assert_eq!(check!(read.read_at(&mut buf, 0)), content.len());
        assert_eq!(str::from_utf8(&buf[..content.len()]), Ok(content));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
        assert_eq!(check!(read.read_at(&mut buf, 14)), 0);
        assert_eq!(check!(read.read_at(&mut buf, 15)), 0);
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
    }
    check!(fs::remove_file(&filename));
}

#[test]
#[cfg(unix)]
fn set_get_unix_permissions() {
    use crate::os::unix::fs::PermissionsExt;

    let tmpdir = tmpdir();
    let filename = &tmpdir.join("set_get_unix_permissions");
    check!(fs::create_dir(filename));
    let mask = 0o7777;

    check!(fs::set_permissions(filename, fs::Permissions::from_mode(0)));
    let metadata0 = check!(fs::metadata(filename));
    assert_eq!(mask & metadata0.permissions().mode(), 0);

    check!(fs::set_permissions(filename, fs::Permissions::from_mode(0o1777)));
    let metadata1 = check!(fs::metadata(filename));
    #[cfg(all(unix, not(target_os = "vxworks")))]
    assert_eq!(mask & metadata1.permissions().mode(), 0o1777);
    #[cfg(target_os = "vxworks")]
    assert_eq!(mask & metadata1.permissions().mode(), 0o0777);
}

#[test]
#[cfg(windows)]
fn file_test_io_seek_read_write() {
    use crate::os::windows::fs::FileExt;

    let tmpdir = tmpdir();
    let filename = tmpdir.join("file_rt_io_file_test_seek_read_write.txt");
    let mut buf = [0; 256];
    let write1 = "asdf";
    let write2 = "qwer-";
    let write3 = "-zxcv";
    let content = "qwer-asdf-zxcv";
    {
        let oo = OpenOptions::new().create_new(true).write(true).read(true).clone();
        let mut rw = check!(oo.open(&filename));
        assert_eq!(check!(rw.seek_write(write1.as_bytes(), 5)), write1.len());
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 9);
        assert_eq!(check!(rw.seek_read(&mut buf, 5)), write1.len());
        assert_eq!(str::from_utf8(&buf[..write1.len()]), Ok(write1));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 9);
        assert_eq!(check!(rw.seek(SeekFrom::Start(0))), 0);
        assert_eq!(check!(rw.write(write2.as_bytes())), write2.len());
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 5);
        assert_eq!(check!(rw.read(&mut buf)), write1.len());
        assert_eq!(str::from_utf8(&buf[..write1.len()]), Ok(write1));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 9);
        assert_eq!(check!(rw.seek_read(&mut buf[..write2.len()], 0)), write2.len());
        assert_eq!(str::from_utf8(&buf[..write2.len()]), Ok(write2));
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 5);
        assert_eq!(check!(rw.seek_write(write3.as_bytes(), 9)), write3.len());
        assert_eq!(check!(rw.seek(SeekFrom::Current(0))), 14);
    }
    {
        let mut read = check!(File::open(&filename));
        assert_eq!(check!(read.seek_read(&mut buf, 0)), content.len());
        assert_eq!(str::from_utf8(&buf[..content.len()]), Ok(content));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
        assert_eq!(check!(read.seek(SeekFrom::End(-5))), 9);
        assert_eq!(check!(read.seek_read(&mut buf, 0)), content.len());
        assert_eq!(str::from_utf8(&buf[..content.len()]), Ok(content));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
        assert_eq!(check!(read.seek(SeekFrom::End(-5))), 9);
        assert_eq!(check!(read.read(&mut buf)), write3.len());
        assert_eq!(str::from_utf8(&buf[..write3.len()]), Ok(write3));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
        assert_eq!(check!(read.seek_read(&mut buf, 0)), content.len());
        assert_eq!(str::from_utf8(&buf[..content.len()]), Ok(content));
        assert_eq!(check!(read.seek(SeekFrom::Current(0))), 14);
        assert_eq!(check!(read.seek_read(&mut buf, 14)), 0);
        assert_eq!(check!(read.seek_read(&mut buf, 15)), 0);
    }
    check!(fs::remove_file(&filename));
}

#[test]
fn file_test_stat_is_correct_on_is_file() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_stat_correct_on_is_file.txt");
    {
        let mut opts = OpenOptions::new();
        let mut fs = check!(opts.read(true).write(true).create(true).open(filename));
        let msg = "hw";
        fs.write(msg.as_bytes()).unwrap();

        let fstat_res = check!(fs.metadata());
        assert!(fstat_res.is_file());
    }
    let stat_res_fn = check!(fs::metadata(filename));
    assert!(stat_res_fn.is_file());
    let stat_res_meth = check!(filename.metadata());
    assert!(stat_res_meth.is_file());
    check!(fs::remove_file(filename));
}

#[test]
fn file_test_stat_is_correct_on_is_dir() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_stat_correct_on_is_dir");
    check!(fs::create_dir(filename));
    let stat_res_fn = check!(fs::metadata(filename));
    assert!(stat_res_fn.is_dir());
    let stat_res_meth = check!(filename.metadata());
    assert!(stat_res_meth.is_dir());
    check!(fs::remove_dir(filename));
}

#[test]
fn file_test_fileinfo_false_when_checking_is_file_on_a_directory() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("fileinfo_false_on_dir");
    check!(fs::create_dir(dir));
    assert!(!dir.is_file());
    check!(fs::remove_dir(dir));
}

#[test]
fn file_test_fileinfo_check_exists_before_and_after_file_creation() {
    let tmpdir = tmpdir();
    let file = &tmpdir.join("fileinfo_check_exists_b_and_a.txt");
    check!(check!(File::create(file)).write(b"foo"));
    assert!(file.exists());
    check!(fs::remove_file(file));
    assert!(!file.exists());
}

#[test]
fn file_test_directoryinfo_check_exists_before_and_after_mkdir() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("before_and_after_dir");
    assert!(!dir.exists());
    check!(fs::create_dir(dir));
    assert!(dir.exists());
    assert!(dir.is_dir());
    check!(fs::remove_dir(dir));
    assert!(!dir.exists());
}

#[test]
fn file_test_directoryinfo_readdir() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("di_readdir");
    check!(fs::create_dir(dir));
    let prefix = "foo";
    for n in 0..3 {
        let f = dir.join(&format!("{}.txt", n));
        let mut w = check!(File::create(&f));
        let msg_str = format!("{}{}", prefix, n.to_string());
        let msg = msg_str.as_bytes();
        check!(w.write(msg));
    }
    let files = check!(fs::read_dir(dir));
    let mut mem = [0; 4];
    for f in files {
        let f = f.unwrap().path();
        {
            let n = f.file_stem().unwrap();
            check!(check!(File::open(&f)).read(&mut mem));
            let read_str = str::from_utf8(&mem).unwrap();
            let expected = format!("{}{}", prefix, n.to_str().unwrap());
            assert_eq!(expected, read_str);
        }
        check!(fs::remove_file(&f));
    }
    check!(fs::remove_dir(dir));
}

#[test]
fn file_create_new_already_exists_error() {
    let tmpdir = tmpdir();
    let file = &tmpdir.join("file_create_new_error_exists");
    check!(fs::File::create(file));
    let e = fs::OpenOptions::new().write(true).create_new(true).open(file).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::AlreadyExists);
}

#[test]
fn mkdir_path_already_exists_error() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("mkdir_error_twice");
    check!(fs::create_dir(dir));
    let e = fs::create_dir(dir).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::AlreadyExists);
}

#[test]
fn recursive_mkdir() {
    let tmpdir = tmpdir();
    let dir = tmpdir.join("d1/d2");
    check!(fs::create_dir_all(&dir));
    assert!(dir.is_dir())
}

#[test]
fn recursive_mkdir_failure() {
    let tmpdir = tmpdir();
    let dir = tmpdir.join("d1");
    let file = dir.join("f1");

    check!(fs::create_dir_all(&dir));
    check!(File::create(&file));

    let result = fs::create_dir_all(&file);

    assert!(result.is_err());
}

#[test]
fn concurrent_recursive_mkdir() {
    for _ in 0..100 {
        let dir = tmpdir();
        let mut dir = dir.join("a");
        for _ in 0..40 {
            dir = dir.join("a");
        }
        let mut join = vec![];
        for _ in 0..8 {
            let dir = dir.clone();
            join.push(thread::spawn(move || {
                check!(fs::create_dir_all(&dir));
            }))
        }

        // No `Display` on result of `join()`
        join.drain(..).map(|join| join.join().unwrap()).for_each(drop);
    }
}

#[test]
fn recursive_mkdir_slash() {
    check!(fs::create_dir_all(Path::new("/")));
}

#[test]
fn recursive_mkdir_dot() {
    check!(fs::create_dir_all(Path::new(".")));
}

#[test]
fn recursive_mkdir_empty() {
    check!(fs::create_dir_all(Path::new("")));
}

#[test]
fn recursive_rmdir() {
    let tmpdir = tmpdir();
    let d1 = tmpdir.join("d1");
    let dt = d1.join("t");
    let dtt = dt.join("t");
    let d2 = tmpdir.join("d2");
    let canary = d2.join("do_not_delete");
    check!(fs::create_dir_all(&dtt));
    check!(fs::create_dir_all(&d2));
    check!(check!(File::create(&canary)).write(b"foo"));
    check!(symlink_junction(&d2, &dt.join("d2")));
    let _ = symlink_file(&canary, &d1.join("canary"));
    check!(fs::remove_dir_all(&d1));

    assert!(!d1.is_dir());
    assert!(canary.exists());
}

#[test]
fn recursive_rmdir_of_symlink() {
    // test we do not recursively delete a symlink but only dirs.
    let tmpdir = tmpdir();
    let link = tmpdir.join("d1");
    let dir = tmpdir.join("d2");
    let canary = dir.join("do_not_delete");
    check!(fs::create_dir_all(&dir));
    check!(check!(File::create(&canary)).write(b"foo"));
    check!(symlink_junction(&dir, &link));
    check!(fs::remove_dir_all(&link));

    assert!(!link.is_dir());
    assert!(canary.exists());
}

#[test]
// only Windows makes a distinction between file and directory symlinks.
#[cfg(windows)]
fn recursive_rmdir_of_file_symlink() {
    let tmpdir = tmpdir();
    if !got_symlink_permission(&tmpdir) {
        return;
    };

    let f1 = tmpdir.join("f1");
    let f2 = tmpdir.join("f2");
    check!(check!(File::create(&f1)).write(b"foo"));
    check!(symlink_file(&f1, &f2));
    match fs::remove_dir_all(&f2) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
}

#[test]
fn unicode_path_is_dir() {
    assert!(Path::new(".").is_dir());
    assert!(!Path::new("test/stdtest/fs.rs").is_dir());

    let tmpdir = tmpdir();

    let mut dirpath = tmpdir.path().to_path_buf();
    dirpath.push("test-가一ー你好");
    check!(fs::create_dir(&dirpath));
    assert!(dirpath.is_dir());

    let mut filepath = dirpath;
    filepath.push("unicode-file-\u{ac00}\u{4e00}\u{30fc}\u{4f60}\u{597d}.rs");
    check!(File::create(&filepath)); // ignore return; touch only
    assert!(!filepath.is_dir());
    assert!(filepath.exists());
}

#[test]
fn unicode_path_exists() {
    assert!(Path::new(".").exists());
    assert!(!Path::new("test/nonexistent-bogus-path").exists());

    let tmpdir = tmpdir();
    let unicode = tmpdir.path();
    let unicode = unicode.join("test-각丁ー再见");
    check!(fs::create_dir(&unicode));
    assert!(unicode.exists());
    assert!(!Path::new("test/unicode-bogus-path-각丁ー再见").exists());
}

#[test]
fn copy_file_does_not_exist() {
    let from = Path::new("test/nonexistent-bogus-path");
    let to = Path::new("test/other-bogus-path");

    match fs::copy(&from, &to) {
        Ok(..) => panic!(),
        Err(..) => {
            assert!(!from.exists());
            assert!(!to.exists());
        }
    }
}

#[test]
fn copy_src_does_not_exist() {
    let tmpdir = tmpdir();
    let from = Path::new("test/nonexistent-bogus-path");
    let to = tmpdir.join("out.txt");
    check!(check!(File::create(&to)).write(b"hello"));
    assert!(fs::copy(&from, &to).is_err());
    assert!(!from.exists());
    let mut v = Vec::new();
    check!(check!(File::open(&to)).read_to_end(&mut v));
    assert_eq!(v, b"hello");
}

#[test]
fn copy_file_ok() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(check!(File::create(&input)).write(b"hello"));
    check!(fs::copy(&input, &out));
    let mut v = Vec::new();
    check!(check!(File::open(&out)).read_to_end(&mut v));
    assert_eq!(v, b"hello");

    assert_eq!(check!(input.metadata()).permissions(), check!(out.metadata()).permissions());
}

#[test]
fn copy_file_dst_dir() {
    let tmpdir = tmpdir();
    let out = tmpdir.join("out");

    check!(File::create(&out));
    match fs::copy(&*out, tmpdir.path()) {
        Ok(..) => panic!(),
        Err(..) => {}
    }
}

#[test]
fn copy_file_dst_exists() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in");
    let output = tmpdir.join("out");

    check!(check!(File::create(&input)).write("foo".as_bytes()));
    check!(check!(File::create(&output)).write("bar".as_bytes()));
    check!(fs::copy(&input, &output));

    let mut v = Vec::new();
    check!(check!(File::open(&output)).read_to_end(&mut v));
    assert_eq!(v, b"foo".to_vec());
}

#[test]
fn copy_file_src_dir() {
    let tmpdir = tmpdir();
    let out = tmpdir.join("out");

    match fs::copy(tmpdir.path(), &out) {
        Ok(..) => panic!(),
        Err(..) => {}
    }
    assert!(!out.exists());
}

#[test]
fn copy_file_preserves_perm_bits() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    let attr = check!(check!(File::create(&input)).metadata());
    let mut p = attr.permissions();
    p.set_readonly(true);
    check!(fs::set_permissions(&input, p));
    check!(fs::copy(&input, &out));
    assert!(check!(out.metadata()).permissions().readonly());
    check!(fs::set_permissions(&input, attr.permissions()));
    check!(fs::set_permissions(&out, attr.permissions()));
}

#[test]
#[cfg(windows)]
fn copy_file_preserves_streams() {
    let tmp = tmpdir();
    check!(check!(File::create(tmp.join("in.txt:bunny"))).write("carrot".as_bytes()));
    assert_eq!(check!(fs::copy(tmp.join("in.txt"), tmp.join("out.txt"))), 0);
    assert_eq!(check!(tmp.join("out.txt").metadata()).len(), 0);
    let mut v = Vec::new();
    check!(check!(File::open(tmp.join("out.txt:bunny"))).read_to_end(&mut v));
    assert_eq!(v, b"carrot".to_vec());
}

#[test]
fn copy_file_returns_metadata_len() {
    let tmp = tmpdir();
    let in_path = tmp.join("in.txt");
    let out_path = tmp.join("out.txt");
    check!(check!(File::create(&in_path)).write(b"lettuce"));
    #[cfg(windows)]
    check!(check!(File::create(tmp.join("in.txt:bunny"))).write(b"carrot"));
    let copied_len = check!(fs::copy(&in_path, &out_path));
    assert_eq!(check!(out_path.metadata()).len(), copied_len);
}

#[test]
fn copy_file_follows_dst_symlink() {
    let tmp = tmpdir();
    if !got_symlink_permission(&tmp) {
        return;
    };

    let in_path = tmp.join("in.txt");
    let out_path = tmp.join("out.txt");
    let out_path_symlink = tmp.join("out_symlink.txt");

    check!(fs::write(&in_path, "foo"));
    check!(fs::write(&out_path, "bar"));
    check!(symlink_file(&out_path, &out_path_symlink));

    check!(fs::copy(&in_path, &out_path_symlink));

    assert!(check!(out_path_symlink.symlink_metadata()).file_type().is_symlink());
    assert_eq!(check!(fs::read(&out_path_symlink)), b"foo".to_vec());
    assert_eq!(check!(fs::read(&out_path)), b"foo".to_vec());
}

#[test]
fn symlinks_work() {
    let tmpdir = tmpdir();
    if !got_symlink_permission(&tmpdir) {
        return;
    };

    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(check!(File::create(&input)).write("foobar".as_bytes()));
    check!(symlink_file(&input, &out));
    assert!(check!(out.symlink_metadata()).file_type().is_symlink());
    assert_eq!(check!(fs::metadata(&out)).len(), check!(fs::metadata(&input)).len());
    let mut v = Vec::new();
    check!(check!(File::open(&out)).read_to_end(&mut v));
    assert_eq!(v, b"foobar".to_vec());
}

#[test]
fn symlink_noexist() {
    // Symlinks can point to things that don't exist
    let tmpdir = tmpdir();
    if !got_symlink_permission(&tmpdir) {
        return;
    };

    // Use a relative path for testing. Symlinks get normalized by Windows,
    // so we may not get the same path back for absolute paths
    check!(symlink_file(&"foo", &tmpdir.join("bar")));
    assert_eq!(check!(fs::read_link(&tmpdir.join("bar"))).to_str().unwrap(), "foo");
}

#[test]
fn read_link() {
    if cfg!(windows) {
        // directory symlink
        assert_eq!(
            check!(fs::read_link(r"C:\Users\All Users")).to_str().unwrap(),
            r"C:\ProgramData"
        );
        // junction
        assert_eq!(
            check!(fs::read_link(r"C:\Users\Default User")).to_str().unwrap(),
            r"C:\Users\Default"
        );
        // junction with special permissions
        assert_eq!(
            check!(fs::read_link(r"C:\Documents and Settings\")).to_str().unwrap(),
            r"C:\Users"
        );
    }
    let tmpdir = tmpdir();
    let link = tmpdir.join("link");
    if !got_symlink_permission(&tmpdir) {
        return;
    };
    check!(symlink_file(&"foo", &link));
    assert_eq!(check!(fs::read_link(&link)).to_str().unwrap(), "foo");
}

#[test]
fn readlink_not_symlink() {
    let tmpdir = tmpdir();
    match fs::read_link(tmpdir.path()) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
}

#[test]
fn links_work() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(check!(File::create(&input)).write("foobar".as_bytes()));
    check!(fs::hard_link(&input, &out));
    assert_eq!(check!(fs::metadata(&out)).len(), check!(fs::metadata(&input)).len());
    assert_eq!(check!(fs::metadata(&out)).len(), check!(input.metadata()).len());
    let mut v = Vec::new();
    check!(check!(File::open(&out)).read_to_end(&mut v));
    assert_eq!(v, b"foobar".to_vec());

    // can't link to yourself
    match fs::hard_link(&input, &input) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
    // can't link to something that doesn't exist
    match fs::hard_link(&tmpdir.join("foo"), &tmpdir.join("bar")) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
}

#[test]
fn chmod_works() {
    let tmpdir = tmpdir();
    let file = tmpdir.join("in.txt");

    check!(File::create(&file));
    let attr = check!(fs::metadata(&file));
    assert!(!attr.permissions().readonly());
    let mut p = attr.permissions();
    p.set_readonly(true);
    check!(fs::set_permissions(&file, p.clone()));
    let attr = check!(fs::metadata(&file));
    assert!(attr.permissions().readonly());

    match fs::set_permissions(&tmpdir.join("foo"), p.clone()) {
        Ok(..) => panic!("wanted an error"),
        Err(..) => {}
    }

    p.set_readonly(false);
    check!(fs::set_permissions(&file, p));
}

#[test]
fn fchmod_works() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("in.txt");

    let file = check!(File::create(&path));
    let attr = check!(fs::metadata(&path));
    assert!(!attr.permissions().readonly());
    let mut p = attr.permissions();
    p.set_readonly(true);
    check!(file.set_permissions(p.clone()));
    let attr = check!(fs::metadata(&path));
    assert!(attr.permissions().readonly());

    p.set_readonly(false);
    check!(file.set_permissions(p));
}

#[test]
fn sync_doesnt_kill_anything() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("in.txt");

    let mut file = check!(File::create(&path));
    check!(file.sync_all());
    check!(file.sync_data());
    check!(file.write(b"foo"));
    check!(file.sync_all());
    check!(file.sync_data());
}

#[test]
fn truncate_works() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("in.txt");

    let mut file = check!(File::create(&path));
    check!(file.write(b"foo"));
    check!(file.sync_all());

    // Do some simple things with truncation
    assert_eq!(check!(file.metadata()).len(), 3);
    check!(file.set_len(10));
    assert_eq!(check!(file.metadata()).len(), 10);
    check!(file.write(b"bar"));
    check!(file.sync_all());
    assert_eq!(check!(file.metadata()).len(), 10);

    let mut v = Vec::new();
    check!(check!(File::open(&path)).read_to_end(&mut v));
    assert_eq!(v, b"foobar\0\0\0\0".to_vec());

    // Truncate to a smaller length, don't seek, and then write something.
    // Ensure that the intermediate zeroes are all filled in (we have `seek`ed
    // past the end of the file).
    check!(file.set_len(2));
    assert_eq!(check!(file.metadata()).len(), 2);
    check!(file.write(b"wut"));
    check!(file.sync_all());
    assert_eq!(check!(file.metadata()).len(), 9);
    let mut v = Vec::new();
    check!(check!(File::open(&path)).read_to_end(&mut v));
    assert_eq!(v, b"fo\0\0\0\0wut".to_vec());
}

#[test]
fn open_flavors() {
    use crate::fs::OpenOptions as OO;
    fn c<T: Clone>(t: &T) -> T {
        t.clone()
    }

    let tmpdir = tmpdir();

    let mut r = OO::new();
    r.read(true);
    let mut w = OO::new();
    w.write(true);
    let mut rw = OO::new();
    rw.read(true).write(true);
    let mut a = OO::new();
    a.append(true);
    let mut ra = OO::new();
    ra.read(true).append(true);

    #[cfg(windows)]
    let invalid_options = 87; // ERROR_INVALID_PARAMETER
    #[cfg(all(unix, not(target_os = "vxworks")))]
    let invalid_options = "Invalid argument";
    #[cfg(target_os = "vxworks")]
    let invalid_options = "invalid argument";

    // Test various combinations of creation modes and access modes.
    //
    // Allowed:
    // creation mode           | read  | write | read-write | append | read-append |
    // :-----------------------|:-----:|:-----:|:----------:|:------:|:-----------:|
    // not set (open existing) |   X   |   X   |     X      |   X    |      X      |
    // create                  |       |   X   |     X      |   X    |      X      |
    // truncate                |       |   X   |     X      |        |             |
    // create and truncate     |       |   X   |     X      |        |             |
    // create_new              |       |   X   |     X      |   X    |      X      |
    //
    // tested in reverse order, so 'create_new' creates the file, and 'open existing' opens it.

    // write-only
    check!(c(&w).create_new(true).open(&tmpdir.join("a")));
    check!(c(&w).create(true).truncate(true).open(&tmpdir.join("a")));
    check!(c(&w).truncate(true).open(&tmpdir.join("a")));
    check!(c(&w).create(true).open(&tmpdir.join("a")));
    check!(c(&w).open(&tmpdir.join("a")));

    // read-only
    error!(c(&r).create_new(true).open(&tmpdir.join("b")), invalid_options);
    error!(c(&r).create(true).truncate(true).open(&tmpdir.join("b")), invalid_options);
    error!(c(&r).truncate(true).open(&tmpdir.join("b")), invalid_options);
    error!(c(&r).create(true).open(&tmpdir.join("b")), invalid_options);
    check!(c(&r).open(&tmpdir.join("a"))); // try opening the file created with write_only

    // read-write
    check!(c(&rw).create_new(true).open(&tmpdir.join("c")));
    check!(c(&rw).create(true).truncate(true).open(&tmpdir.join("c")));
    check!(c(&rw).truncate(true).open(&tmpdir.join("c")));
    check!(c(&rw).create(true).open(&tmpdir.join("c")));
    check!(c(&rw).open(&tmpdir.join("c")));

    // append
    check!(c(&a).create_new(true).open(&tmpdir.join("d")));
    error!(c(&a).create(true).truncate(true).open(&tmpdir.join("d")), invalid_options);
    error!(c(&a).truncate(true).open(&tmpdir.join("d")), invalid_options);
    check!(c(&a).create(true).open(&tmpdir.join("d")));
    check!(c(&a).open(&tmpdir.join("d")));

    // read-append
    check!(c(&ra).create_new(true).open(&tmpdir.join("e")));
    error!(c(&ra).create(true).truncate(true).open(&tmpdir.join("e")), invalid_options);
    error!(c(&ra).truncate(true).open(&tmpdir.join("e")), invalid_options);
    check!(c(&ra).create(true).open(&tmpdir.join("e")));
    check!(c(&ra).open(&tmpdir.join("e")));

    // Test opening a file without setting an access mode
    let mut blank = OO::new();
    error!(blank.create(true).open(&tmpdir.join("f")), invalid_options);

    // Test write works
    check!(check!(File::create(&tmpdir.join("h"))).write("foobar".as_bytes()));

    // Test write fails for read-only
    check!(r.open(&tmpdir.join("h")));
    {
        let mut f = check!(r.open(&tmpdir.join("h")));
        assert!(f.write("wut".as_bytes()).is_err());
    }

    // Test write overwrites
    {
        let mut f = check!(c(&w).open(&tmpdir.join("h")));
        check!(f.write("baz".as_bytes()));
    }
    {
        let mut f = check!(c(&r).open(&tmpdir.join("h")));
        let mut b = vec![0; 6];
        check!(f.read(&mut b));
        assert_eq!(b, "bazbar".as_bytes());
    }

    // Test truncate works
    {
        let mut f = check!(c(&w).truncate(true).open(&tmpdir.join("h")));
        check!(f.write("foo".as_bytes()));
    }
    assert_eq!(check!(fs::metadata(&tmpdir.join("h"))).len(), 3);

    // Test append works
    assert_eq!(check!(fs::metadata(&tmpdir.join("h"))).len(), 3);
    {
        let mut f = check!(c(&a).open(&tmpdir.join("h")));
        check!(f.write("bar".as_bytes()));
    }
    assert_eq!(check!(fs::metadata(&tmpdir.join("h"))).len(), 6);

    // Test .append(true) equals .write(true).append(true)
    {
        let mut f = check!(c(&w).append(true).open(&tmpdir.join("h")));
        check!(f.write("baz".as_bytes()));
    }
    assert_eq!(check!(fs::metadata(&tmpdir.join("h"))).len(), 9);
}

#[test]
fn _assert_send_sync() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<OpenOptions>();
}

#[test]
fn binary_file() {
    let mut bytes = [0; 1024];
    StdRng::from_entropy().fill_bytes(&mut bytes);

    let tmpdir = tmpdir();

    check!(check!(File::create(&tmpdir.join("test"))).write(&bytes));
    let mut v = Vec::new();
    check!(check!(File::open(&tmpdir.join("test"))).read_to_end(&mut v));
    assert!(v == &bytes[..]);
}

#[test]
fn write_then_read() {
    let mut bytes = [0; 1024];
    StdRng::from_entropy().fill_bytes(&mut bytes);

    let tmpdir = tmpdir();

    check!(fs::write(&tmpdir.join("test"), &bytes[..]));
    let v = check!(fs::read(&tmpdir.join("test")));
    assert!(v == &bytes[..]);

    check!(fs::write(&tmpdir.join("not-utf8"), &[0xFF]));
    error_contains!(
        fs::read_to_string(&tmpdir.join("not-utf8")),
        "stream did not contain valid UTF-8"
    );

    let s = "𐁁𐀓𐀠𐀴𐀍";
    check!(fs::write(&tmpdir.join("utf8"), s.as_bytes()));
    let string = check!(fs::read_to_string(&tmpdir.join("utf8")));
    assert_eq!(string, s);
}

#[test]
fn file_try_clone() {
    let tmpdir = tmpdir();

    let mut f1 =
        check!(OpenOptions::new().read(true).write(true).create(true).open(&tmpdir.join("test")));
    let mut f2 = check!(f1.try_clone());

    check!(f1.write_all(b"hello world"));
    check!(f1.seek(SeekFrom::Start(2)));

    let mut buf = vec![];
    check!(f2.read_to_end(&mut buf));
    assert_eq!(buf, b"llo world");
    drop(f2);

    check!(f1.write_all(b"!"));
}

#[test]
#[cfg(not(windows))]
fn unlink_readonly() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("file");
    check!(File::create(&path));
    let mut perm = check!(fs::metadata(&path)).permissions();
    perm.set_readonly(true);
    check!(fs::set_permissions(&path, perm));
    check!(fs::remove_file(&path));
}

#[test]
fn mkdir_trailing_slash() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("file");
    check!(fs::create_dir_all(&path.join("a/")));
}

#[test]
fn canonicalize_works_simple() {
    let tmpdir = tmpdir();
    let tmpdir = fs::canonicalize(tmpdir.path()).unwrap();
    let file = tmpdir.join("test");
    File::create(&file).unwrap();
    assert_eq!(fs::canonicalize(&file).unwrap(), file);
}

#[test]
fn realpath_works() {
    let tmpdir = tmpdir();
    if !got_symlink_permission(&tmpdir) {
        return;
    };

    let tmpdir = fs::canonicalize(tmpdir.path()).unwrap();
    let file = tmpdir.join("test");
    let dir = tmpdir.join("test2");
    let link = dir.join("link");
    let linkdir = tmpdir.join("test3");

    File::create(&file).unwrap();
    fs::create_dir(&dir).unwrap();
    symlink_file(&file, &link).unwrap();
    symlink_dir(&dir, &linkdir).unwrap();

    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());

    assert_eq!(fs::canonicalize(&tmpdir).unwrap(), tmpdir);
    assert_eq!(fs::canonicalize(&file).unwrap(), file);
    assert_eq!(fs::canonicalize(&link).unwrap(), file);
    assert_eq!(fs::canonicalize(&linkdir).unwrap(), dir);
    assert_eq!(fs::canonicalize(&linkdir.join("link")).unwrap(), file);
}

#[test]
fn realpath_works_tricky() {
    let tmpdir = tmpdir();
    if !got_symlink_permission(&tmpdir) {
        return;
    };

    let tmpdir = fs::canonicalize(tmpdir.path()).unwrap();
    let a = tmpdir.join("a");
    let b = a.join("b");
    let c = b.join("c");
    let d = a.join("d");
    let e = d.join("e");
    let f = a.join("f");

    fs::create_dir_all(&b).unwrap();
    fs::create_dir_all(&d).unwrap();
    File::create(&f).unwrap();
    if cfg!(not(windows)) {
        symlink_file("../d/e", &c).unwrap();
        symlink_file("../f", &e).unwrap();
    }
    if cfg!(windows) {
        symlink_file(r"..\d\e", &c).unwrap();
        symlink_file(r"..\f", &e).unwrap();
    }

    assert_eq!(fs::canonicalize(&c).unwrap(), f);
    assert_eq!(fs::canonicalize(&e).unwrap(), f);
}

#[test]
fn dir_entry_methods() {
    let tmpdir = tmpdir();

    fs::create_dir_all(&tmpdir.join("a")).unwrap();
    File::create(&tmpdir.join("b")).unwrap();

    for file in tmpdir.path().read_dir().unwrap().map(|f| f.unwrap()) {
        let fname = file.file_name();
        match fname.to_str() {
            Some("a") => {
                assert!(file.file_type().unwrap().is_dir());
                assert!(file.metadata().unwrap().is_dir());
            }
            Some("b") => {
                assert!(file.file_type().unwrap().is_file());
                assert!(file.metadata().unwrap().is_file());
            }
            f => panic!("unknown file name: {:?}", f),
        }
    }
}

#[test]
fn dir_entry_debug() {
    let tmpdir = tmpdir();
    File::create(&tmpdir.join("b")).unwrap();
    let mut read_dir = tmpdir.path().read_dir().unwrap();
    let dir_entry = read_dir.next().unwrap().unwrap();
    let actual = format!("{:?}", dir_entry);
    let expected = format!("DirEntry({:?})", dir_entry.0.path());
    assert_eq!(actual, expected);
}

#[test]
fn read_dir_not_found() {
    let res = fs::read_dir("/path/that/does/not/exist");
    assert_eq!(res.err().unwrap().kind(), ErrorKind::NotFound);
}

#[test]
fn create_dir_all_with_junctions() {
    let tmpdir = tmpdir();
    let target = tmpdir.join("target");

    let junction = tmpdir.join("junction");
    let b = junction.join("a/b");

    let link = tmpdir.join("link");
    let d = link.join("c/d");

    fs::create_dir(&target).unwrap();

    check!(symlink_junction(&target, &junction));
    check!(fs::create_dir_all(&b));
    // the junction itself is not a directory, but `is_dir()` on a Path
    // follows links
    assert!(junction.is_dir());
    assert!(b.exists());

    if !got_symlink_permission(&tmpdir) {
        return;
    };
    check!(symlink_dir(&target, &link));
    check!(fs::create_dir_all(&d));
    assert!(link.is_dir());
    assert!(d.exists());
}

#[test]
fn metadata_access_times() {
    let tmpdir = tmpdir();

    let b = tmpdir.join("b");
    File::create(&b).unwrap();

    let a = check!(fs::metadata(&tmpdir.path()));
    let b = check!(fs::metadata(&b));

    assert_eq!(check!(a.accessed()), check!(a.accessed()));
    assert_eq!(check!(a.modified()), check!(a.modified()));
    assert_eq!(check!(b.accessed()), check!(b.modified()));

    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        check!(a.created());
        check!(b.created());
    }

    if cfg!(target_os = "linux") {
        // Not always available
        match (a.created(), b.created()) {
            (Ok(t1), Ok(t2)) => assert!(t1 <= t2),
            (Err(e1), Err(e2))
                if e1.kind() == ErrorKind::Other && e2.kind() == ErrorKind::Other => {}
            (a, b) => {
                panic!("creation time must be always supported or not supported: {:?} {:?}", a, b,)
            }
        }
    }
}

/// Test creating hard links to symlinks.
#[test]
fn symlink_hard_link() {
    let tmpdir = tmpdir();
    if !got_symlink_permission(&tmpdir) {
        return;
    };

    // Create "file", a file.
    check!(fs::File::create(tmpdir.join("file")));

    // Create "symlink", a symlink to "file".
    check!(symlink_file("file", tmpdir.join("symlink")));

    // Create "hard_link", a hard link to "symlink".
    check!(fs::hard_link(tmpdir.join("symlink"), tmpdir.join("hard_link")));

    // "hard_link" should appear as a symlink.
    assert!(check!(fs::symlink_metadata(tmpdir.join("hard_link"))).file_type().is_symlink());

    // We sould be able to open "file" via any of the above names.
    let _ = check!(fs::File::open(tmpdir.join("file")));
    assert!(fs::File::open(tmpdir.join("file.renamed")).is_err());
    let _ = check!(fs::File::open(tmpdir.join("symlink")));
    let _ = check!(fs::File::open(tmpdir.join("hard_link")));

    // Rename "file" to "file.renamed".
    check!(fs::rename(tmpdir.join("file"), tmpdir.join("file.renamed")));

    // Now, the symlink and the hard link should be dangling.
    assert!(fs::File::open(tmpdir.join("file")).is_err());
    let _ = check!(fs::File::open(tmpdir.join("file.renamed")));
    assert!(fs::File::open(tmpdir.join("symlink")).is_err());
    assert!(fs::File::open(tmpdir.join("hard_link")).is_err());

    // The symlink and the hard link should both still point to "file".
    assert!(fs::read_link(tmpdir.join("file")).is_err());
    assert!(fs::read_link(tmpdir.join("file.renamed")).is_err());
    assert_eq!(check!(fs::read_link(tmpdir.join("symlink"))), Path::new("file"));
    assert_eq!(check!(fs::read_link(tmpdir.join("hard_link"))), Path::new("file"));

    // Remove "file.renamed".
    check!(fs::remove_file(tmpdir.join("file.renamed")));

    // Now, we can't open the file by any name.
    assert!(fs::File::open(tmpdir.join("file")).is_err());
    assert!(fs::File::open(tmpdir.join("file.renamed")).is_err());
    assert!(fs::File::open(tmpdir.join("symlink")).is_err());
    assert!(fs::File::open(tmpdir.join("hard_link")).is_err());

    // "hard_link" should still appear as a symlink.
    assert!(check!(fs::symlink_metadata(tmpdir.join("hard_link"))).file_type().is_symlink());
}
