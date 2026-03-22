pub(super) struct MavenCoord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: Option<String>,
}

impl MavenCoord {
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
