use std::{fs, path::PathBuf, ptr::null_mut};

use freetype::freetype as ft;
use syn::LitStr;

pub(crate) struct FreeTypeFont {
    library: ft::FT_Library,
    face: ft::FT_Face,
    _bytes: Vec<u8>,
}

impl FreeTypeFont {
    pub(crate) fn face(&self) -> ft::FT_Face {
        self.face
    }

    pub(crate) fn set_pixel_size(&self, height: u16) -> Result<(), String> {
        ft_ok(
            unsafe { ft::FT_Set_Char_Size(self.face, 0, height as ft::FT_F26Dot6 * 64, 300, 300) },
            "set character size",
        )?;
        ft_ok(
            unsafe { ft::FT_Set_Pixel_Sizes(self.face, 0, height as u32) },
            "set pixel size",
        )
    }
}

impl Drop for FreeTypeFont {
    fn drop(&mut self) {
        unsafe {
            if !self.face.is_null() {
                let _ = ft::FT_Done_Face(self.face);
            }
            if !self.library.is_null() {
                let _ = ft::FT_Done_FreeType(self.library);
            }
        }
    }
}

pub(crate) fn load_font(path: &LitStr) -> syn::Result<FreeTypeFont> {
    let font_path = resolve_font_path(&path.value()).map_err(|message| {
        syn::Error::new(
            path.span(),
            format!("failed to resolve font path {:?}: {message}", path.value()),
        )
    })?;

    let bytes = fs::read(&font_path).map_err(|err| {
        syn::Error::new(
            path.span(),
            format!("failed to read font path {}: {err}", font_path.display()),
        )
    })?;

    load_freetype_face(bytes).map_err(|err| syn::Error::new(path.span(), err))
}

fn load_freetype_face(bytes: Vec<u8>) -> Result<FreeTypeFont, String> {
    let mut library = null_mut();
    ft_ok(
        unsafe { ft::FT_Init_FreeType(&mut library) },
        "initialize FreeType",
    )?;

    let mut face = null_mut();
    let face_result = ft_ok(
        unsafe {
            ft::FT_New_Memory_Face(
                library,
                bytes.as_ptr(),
                bytes.len() as ft::FT_Long,
                0,
                &mut face,
            )
        },
        "parse font",
    );

    if let Err(err) = face_result {
        unsafe {
            let _ = ft::FT_Done_FreeType(library);
        }
        return Err(err);
    }

    Ok(FreeTypeFont {
        library,
        face,
        _bytes: bytes,
    })
}

pub(crate) fn ft_ok(error: ft::FT_Error, action: &str) -> Result<(), String> {
    if error == ft::FT_Err_Ok as ft::FT_Error {
        Ok(())
    } else {
        Err(format!("failed to {action}: FreeType error {error}"))
    }
}

fn resolve_font_path(path: &str) -> Result<PathBuf, String> {
    let literal = PathBuf::from(path);
    if literal.is_absolute() && literal.exists() {
        return Ok(literal);
    }

    font_path_candidates(&literal)
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            "tried current directory, caller CARGO_MANIFEST_DIR, macro crate, and workspace root"
                .into()
        })
}

fn font_path_candidates(path: &PathBuf) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(path));
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest_dir).join(path));
    }

    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path));
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path),
    );
    candidates
}
