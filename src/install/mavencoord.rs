/// Parsed Maven coordinate for library artifact resolution.
///
/// Represents a Maven coordinate in the format `group:artifact:version[:classifier][@extension]`,
/// and can convert it to a file path for local library storage.
///
/// # Examples
///
/// ```text
/// net.neoforged:neoforge:21.10.37
/// org.lwjgl:lwjgl:3.3.3:natives-linux@zip
/// ```
pub(super) struct MavenCoord {
    /// Maven group ID (e.g., `net.neoforged`).
    pub group: String,
    /// Maven artifact ID (e.g., `neoforge`).
    pub artifact: String,
    /// Artifact version (e.g., `21.10.37`).
    pub version: String,
    /// Optional classifier (e.g., `natives-linux`, `installer`).
    pub classifier: Option<String>,
    /// Optional file extension (defaults to `jar` if not specified).
    pub extension: Option<String>,
}

impl MavenCoord {
    /// Parses a Maven coordinate string into its components.
    ///
    /// Supports the following formats:
    /// - `group:artifact:version`
    /// - `group:artifact:version:classifier`
    /// - `group:artifact:version@extension`
    /// - `group:artifact:version:classifier@extension`
    pub fn parse(coord: &str) -> Self {
        let mut extension = None;
        let base_coord = match coord.split_once('@') {
            Some((prev, ext)) => {
                extension = Some(ext.to_string());
                prev
            }
            None => coord,
        };
        let parts: Vec<_> = base_coord.split(':').collect();
        let group = parts[0].to_string();
        let artifact = parts[1].to_string();
        let version = parts[2].to_string();
        let classifier = parts.get(3).map(std::string::ToString::to_string);

        Self {
            group,
            artifact,
            version,
            classifier,
            extension,
        }
    }

    /// Converts the Maven coordinate to a repository file path.
    ///
    /// Returns a path in the format:
    /// `{group_path}/{artifact}/{version}/{artifact}-{version}[-classifier].{ext}`
    ///
    /// The group ID dots are replaced with path separators.
    /// The extension defaults to `jar` if not specified.
    pub fn to_path_string(&self) -> String {
        let group_path = self.group.replace('.', "/");
        let extension = self.extension.as_deref().unwrap_or("jar");
        let filename = match self.classifier.as_deref() {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, extension),
            None => format!("{}-{}.{}", self.artifact, self.version, extension),
        };
        format!(
            "{}/{}/{}/{}",
            group_path, self.artifact, self.version, filename
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Basic: group:artifact:version
    #[test]
    fn basic() {
        let coord = MavenCoord::parse("net.neoforged:neoforge:21.10.37");
        assert_eq!(coord.group, "net.neoforged");
        assert_eq!(coord.artifact, "neoforge");
        assert_eq!(coord.version, "21.10.37");
        assert_eq!(coord.classifier, None);
        assert_eq!(coord.extension, None);
    }
    // Group with multiple dots
    #[test]
    fn multi_dot_group() {
        let coord = MavenCoord::parse("org.lwjgl:lwjgl:3.3.3");
        assert_eq!(coord.group, "org.lwjgl");
        assert_eq!(coord.artifact, "lwjgl");
        assert_eq!(coord.version, "3.3.3");
    }
    // With classifier only
    #[test]
    fn with_classifier() {
        let coord = MavenCoord::parse("net.neoforged:neoforge:21.10.37:installer");
        assert_eq!(coord.version, "21.10.37");
        assert_eq!(coord.classifier, Some("installer".into()));
        assert_eq!(coord.extension, None);
    }
    // With extension only
    #[test]
    fn with_extension() {
        let coord = MavenCoord::parse("org.lwjgl:lwjgl:3.3.3@zip");
        assert_eq!(coord.group, "org.lwjgl");
        assert_eq!(coord.classifier, None);
        assert_eq!(coord.extension, Some("zip".into()));
    }
    // With classifier and extension
    #[test]
    fn with_classifier_and_extension() {
        let coord = MavenCoord::parse("org.lwjgl:lwjgl:3.3.3:natives-linux@zip");
        assert_eq!(coord.classifier, Some("natives-linux".into()));
        assert_eq!(coord.extension, Some("zip".into()));
    }
    // Classifier with jar extension
    #[test]
    fn classifier_with_jar_extension() {
        let coord = MavenCoord::parse("net.neoforged:neoforge:21.10.37:installer@jar");
        assert_eq!(coord.classifier, Some("installer".into()));
        assert_eq!(coord.extension, Some("jar".into()));
    }
}
