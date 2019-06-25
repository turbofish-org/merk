#![macro_use]

macro_rules! assert_err {
    ($result:ident, $kind:path) => {
        match $result {
            Err(err) => {
                match err.kind() {
                    $kind(ref _kind) => {},
                    _ => panic!("Unexpected error kind")
                }
            },
            _ => panic!("Expected Err, got Ok")
        }
    };

    ($result:ident, $message:expr) => {
        match $result {
            Err(err) => {
                assert_eq!(err.description(), $message);
            },
            _ => panic!("Expected Err, got Ok")
        }
    };
}
