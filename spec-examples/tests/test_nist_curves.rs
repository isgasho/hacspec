use hacspec_examples::ec::{arithmetic, p256, p384, Affine};

use hacspec::prelude::*;
use hacspec_dev::prelude::*;

create_test_vectors!(
    TestVector,
    algorithm: String,
    generatorVersion: String,
    numberOfTests: usize,
    header: Vec<Value>,   // not used
    notes: Option<Value>, // text notes (might not be present), keys correspond to flags
    schema: String,       // not used
    testGroups: Vec<TestGroup>
);

create_test_vectors!(
    TestGroup,
    curve: String,
    encoding: String,
    r#type: String,
    tests: Vec<Test>
);

create_test_vectors!(
    Test,
    tcId: usize,
    comment: String,
    public: String,
    private: String,
    shared: String,
    result: String,
    flags: Vec<String>
);

#[allow(non_snake_case)]
fn run_test<Scalar: UnsignedIntegerCopy, FieldElement: UnsignedIntegerCopy>(
    tests: TestVector,
    curve: &'static str,
) {
    let num_tests = tests.numberOfTests;
    let mut skipped_tests = 0;
    let mut tests_run = 0;
    match tests.algorithm.as_ref() {
        "ECDH" => (),
        _ => panic!("This is not an ECDH test vector."),
    };
    for testGroup in tests.testGroups.iter() {
        assert_eq!(testGroup.r#type, "EcdhEcpointTest");
        assert_eq!(testGroup.curve, curve);
        assert_eq!(testGroup.encoding, "ecpoint");
        let point_len = match curve {
            "secp256r1" => 64,
            "secp384r1" => 96,
            _ => panic!("I don't know that curve"),
        };
        for test in testGroup.tests.iter() {
            println!("Test {:?}: {:?}", test.tcId, test.comment);
            if !test.result.eq("valid") {
                println!("We're only doing valid tests for now.");
                skipped_tests += 1;
                continue;
            }
            if test.comment == "compressed public key" {
                // not implemented
                println!("Compressed public keys are not supported.");
                skipped_tests += 1;
                continue;
            }
            assert_eq!(&test.public[0..2], "04");
            let k = Scalar::from_hex_string(&test.private);
            // println!("k: {:?}", k);
            let p = Affine(
                FieldElement::from_hex_string(&test.public[2..point_len + 2].to_string()),
                FieldElement::from_hex_string(&test.public[point_len + 2..].to_string()),
            );
            // println!("p: {:?}", p);
            let expected = FieldElement::from_hex_string(&test.shared);
            // println!("expected: {:?}", expected);
            let shared = arithmetic::point_mul(k, p);
            // println!("computed: {:?}", shared);
            assert!(shared.0.equal(expected));
            tests_run += 1;
        }
    }
    // Check that we ran all tests.
    println!(
        "Ran {} out of {} tests and skipped {}.",
        tests_run, num_tests, skipped_tests
    );
    assert_eq!(num_tests - skipped_tests, tests_run);
}

#[test]
fn test_wycheproof_384_plain() {
    let tests: TestVector = TestVector::from_file("tests/ecdh_secp384r1_ecpoint_test.json");
    run_test::<p384::Scalar, p384::FieldElement>(tests, "secp384r1");
}

#[test]
fn test_wycheproof_256_plain() {
    let tests: TestVector = TestVector::from_file("tests/ecdh_secp256r1_ecpoint_test.json");
    run_test::<p256::Scalar, p256::FieldElement>(tests, "secp256r1");
}
