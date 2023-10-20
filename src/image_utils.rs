#[derive(Debug, PartialEq)]
pub enum FileType {
    JPEG,
    PNG,
    WEBP,
    HEIF,
}

pub struct FileIdentification {
    file_type: FileType,
    file_extension: &'static str,
    file_header: &'static [u8],
}

const FILE_MAPPINGS: [FileIdentification; 4] = [
    FileIdentification {
        file_type: FileType::JPEG,
        file_extension: "jpg",
        file_header: &[0xff, 0xd8, 0xff],
    },
    FileIdentification {
        file_type: FileType::PNG,
        file_extension: "png",
        file_header: &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    },
    FileIdentification {
        file_type: FileType::WEBP,
        file_extension: "webp",
        file_header: &[0x52, 0x49, 0x46, 0x46],
    },
    FileIdentification {
        file_type: FileType::HEIF,
        file_extension: "heic",
        file_header: &[0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63],
    },
];

/*

func isImageValid(image []byte) bool {
	for _, magicNumber := range magicNumbers {
		if isMagicNumberValid(image, magicNumber) {
			return true
		}
	}
	return false
}

func isMagicNumberValid(image []byte, magicNumber []byte) bool {
	if len(image) < len(magicNumber) {
		return false
	}

	for i := range magicNumber {
		if image[i] != magicNumber[i] {
			return false
		}
	}
	return true
}

 */
pub fn determine_file_type(image: &[u8]) -> Option<&FileIdentification> {
    FILE_MAPPINGS.iter().find(|&mapping| image.starts_with(mapping.file_header))
}