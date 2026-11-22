// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Ids are stable once shipped: append only.

pub const NAMES: &[&str] = &[
    "Other",
    "Rust",
    "TypeScript",
    "JavaScript",
    "Python",
    "Java",
    "Kotlin",
    "Go",
    "C",
    "C++",
    "C#",
    "Swift",
    "Dart",
    "Ruby",
    "PHP",
    "Lua",
    "Shell",
    "SQL",
    "HTML",
    "CSS",
    "Vue",
    "Svelte",
    "Markdown",
    "Config",
    "Shaders",
    "Minecraft data",
];

pub const MARKDOWN: u8 = 22;
pub const CONFIG: u8 = 23;

const FILES: &[(&str, u8)] = &[
    ("cargo.toml", 1),
    ("package.json", 3),
    ("pyproject.toml", 4),
    ("requirements.txt", 4),
    ("pom.xml", 5),
    ("build.gradle", 6),
    ("build.gradle.kts", 6),
    ("settings.gradle", 6),
    ("settings.gradle.kts", 6),
    ("go.mod", 7),
    ("cmakelists.txt", 9),
    ("pubspec.yaml", 12),
    ("gemfile", 13),
];

const EXTS: &[(&str, u8)] = &[
    ("rs", 1),
    ("ts", 2),
    ("tsx", 2),
    ("mts", 2),
    ("cts", 2),
    ("js", 3),
    ("jsx", 3),
    ("mjs", 3),
    ("cjs", 3),
    ("py", 4),
    ("pyi", 4),
    ("ipynb", 4),
    ("java", 5),
    ("kt", 6),
    ("kts", 6),
    ("go", 7),
    ("c", 8),
    ("h", 8),
    ("cpp", 9),
    ("cc", 9),
    ("cxx", 9),
    ("hpp", 9),
    ("hh", 9),
    ("hxx", 9),
    ("cs", 10),
    ("csproj", 10),
    ("sln", 10),
    ("swift", 11),
    ("dart", 12),
    ("rb", 13),
    ("php", 14),
    ("lua", 15),
    ("sh", 16),
    ("bash", 16),
    ("zsh", 16),
    ("ps1", 16),
    ("psm1", 16),
    ("bat", 16),
    ("cmd", 16),
    ("sql", 17),
    ("html", 18),
    ("htm", 18),
    ("css", 19),
    ("scss", 19),
    ("sass", 19),
    ("less", 19),
    ("vue", 20),
    ("svelte", 21),
    ("md", 22),
    ("mdx", 22),
    ("json", 23),
    ("yaml", 23),
    ("yml", 23),
    ("toml", 23),
    ("ini", 23),
    ("glsl", 24),
    ("frag", 24),
    ("vert", 24),
    ("wgsl", 24),
    ("hlsl", 24),
    ("mcmeta", 25),
    ("mcfunction", 25),
];

pub fn of_path(path: &str) -> Option<u8> {
    let name = path.rsplit(['/', '\\']).next()?.to_ascii_lowercase();
    if let Some(&(_, id)) = FILES.iter().find(|(f, _)| *f == name) {
        return Some(id);
    }
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }
    Some(
        EXTS.iter()
            .find(|(e, _)| *e == ext)
            .map_or(0, |&(_, id)| id),
    )
}

pub fn name(id: u8) -> &'static str {
    NAMES.get(usize::from(id)).copied().unwrap_or(NAMES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_map_by_build_file_then_extension() {
        assert_eq!(of_path("D:\\w\\crate\\Cargo.toml").map(name), Some("Rust"));
        assert_eq!(
            of_path("/w/app/settings.gradle.kts").map(name),
            Some("Kotlin")
        );
        assert_eq!(of_path("/w/x.config.toml").map(name), Some("Config"));
        assert_eq!(of_path("src/Main.TSX").map(name), Some("TypeScript"));
        assert_eq!(of_path("/w/x.unknownext"), Some(0));
        assert_eq!(of_path("/w/Makefile"), None);
        assert_eq!(of_path("/w/.gitignore"), None);
    }
}
