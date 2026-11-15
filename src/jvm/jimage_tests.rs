use super::jimage::ModulesImage;
use super::string_sharing::restore_shared_strings;

// `lib/modules` con una clase sin comprimir, una comprimida con `zip`, una con un descompresor que no
// se conoce y un recurso que no es una clase.
fn modules_image() -> Vec<u8> {
    let strings = b"\0java/lang\0Hola\0class\0Comprimida\0txt\0zip\0Rota\0lz4-nuevo\0";
    let (parent, hola, class, compressed, text, zip, broken, unknown) = (1, 11, 16, 22, 33, 37, 41, 46);
    let class_bytes = b"\xCA\xFE\xBA\xBE";
    let packed = |decompressor: u32| {
        let payload = miniz_oxide::deflate::compress_to_vec_zlib(class_bytes, 6);
        let mut stored = Vec::new();
        stored.extend(0xCAFE_FAFA_u32.to_le_bytes());
        stored.extend(u64::try_from(payload.len()).expect("entra").to_le_bytes());
        stored.extend(u64::try_from(class_bytes.len()).expect("entra").to_le_bytes());
        stored.extend(decompressor.to_le_bytes());
        stored.extend(0_u32.to_le_bytes());
        stored.push(1);
        stored.extend(payload);
        stored
    };
    let (zipped, unreadable) = (packed(zip), packed(unknown));
    let size = |bytes: &[u8]| u8::try_from(bytes.len()).expect("entra en un byte");
    let location = |base: u8, extension: u8, (offset, stored): (u8, u8)| {
        vec![
            0b0001_0000,
            parent,
            0b0001_1000,
            base,
            0b0010_0000,
            extension,
            0b0010_1000,
            offset,
            0b0011_0000,
            stored,
            0b0011_1000,
            4,
            0,
        ]
    };
    let zipped_at = size(class_bytes);
    let unreadable_at = zipped_at + size(&zipped);
    let locations = [
        location(hola, class, (0, 0)),
        location(compressed, class, (zipped_at, size(&zipped))),
        location(broken, class, (unreadable_at, size(&unreadable))),
        location(hola, text, (0, 0)),
    ];
    let offsets: Vec<u32> = vec![0, 13, 26, 39];
    let locations_size: u32 = 52;
    let mut image = Vec::new();
    let header = [0xCAFE_DADA, 0x0001_0000, 0, 4, 4, locations_size, 56];
    image.extend(header.iter().flat_map(|slot: &u32| slot.to_le_bytes()));
    image.extend([0_u8; 16]);
    image.extend(offsets.iter().flat_map(|offset| offset.to_le_bytes()));
    image.extend(locations.iter().flatten());
    image.extend(strings);
    image.extend(class_bytes);
    image.extend(zipped);
    image.extend(unreadable);
    image
}

#[test]
fn the_modules_image_reads_plain_and_zip_compressed_classes() {
    use crate::test_support::TempTree;
    let tree = TempTree::new("jvm-jimage");
    let path = tree.path().join("modules");
    std::fs::write(&path, modules_image()).expect("se escribe");
    let image = ModulesImage::open(&path).expect("la imagen se abre");
    for class in ["java/lang/Hola.class", "java/lang/Comprimida.class"] {
        assert_eq!(image.read(class).as_deref(), Some(&b"\xCA\xFE\xBA\xBE"[..]));
    }
    assert!(image.read("java/lang/Rota.class").is_none());
    assert!(!image.contains("java/lang/Hola.txt"));
    assert!(image.read("no/Existe.class").is_none());
    std::fs::write(&path, b"no es una imagen de jimage, ni de lejos").expect("se escribe");
    assert!(ModulesImage::open(&path).is_none());
}

// Una clase de `compact-cp`: un texto externalizado, uno propio, un descriptor armado con su esqueleto
// y sus clases, y una constante que pasa igual; lo que sigue al pool queda como está.
#[test]
fn shared_strings_are_restored_into_the_constant_pool() {
    let strings = b"\0java/lang\0String\0(L;)V\0Hola\0";
    let mut stored = b"\xCA\xFE\xBA\xBE\x00\x00\x00\x41\x00\x05".to_vec();
    stored.extend([23, 0xB8]);
    stored.extend([1, 0, 2, b'O', b'k']);
    stored.extend([25, 0xB2, 0xA2, 0xA1, 0xAB]);
    stored.extend([7, 0, 2]);
    stored.extend([0x00, 0x21]);
    let mut expected = b"\xCA\xFE\xBA\xBE\x00\x00\x00\x41\x00\x05".to_vec();
    expected.extend([1, 0, 4]);
    expected.extend(b"Hola");
    expected.extend([1, 0, 2, b'O', b'k']);
    expected.extend([1, 0, 21]);
    expected.extend(b"(Ljava/lang/String;)V");
    expected.extend([7, 0, 2, 0x00, 0x21]);
    assert_eq!(restore_shared_strings(&stored, strings), Some(expected));
    assert!(restore_shared_strings(&stored[..12], strings).is_none());
}
