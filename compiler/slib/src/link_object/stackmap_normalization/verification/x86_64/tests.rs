use super::*;

#[test]
fn exact_direct_register_and_memory_call_return_boundaries() {
    let code = [
        0x55, 0x48, 0x89, 0xe5, // push rbp; mov rbp,rsp
        0x48, 0x83, 0xec, 0x10, // sub rsp,16
        0xe8, 0, 0, 0, 0, // call rel32
        0xff, 0xd0, // call rax
        0xff, 0x15, 0, 0, 0, 0, // call [rip+disp32]
    ];
    validate_calls(&code, &[13, 15, 21]).unwrap();
    assert_eq!(
        validate_calls(&code, &[0]),
        Err(Error::ReturnPcOutsideFunction)
    );
    assert_eq!(
        validate_calls(&code, &[22]),
        Err(Error::ReturnPcOutsideFunction)
    );
    assert_eq!(
        validate_calls(&code, &[12, 21]),
        Err(Error::ReturnPcDoesNotFollowCall { offset: 12 })
    );
    assert!(matches!(
        validate_calls(&code, &[20]),
        Err(Error::InvalidInstruction { .. })
    ));
}

#[test]
fn opcode_bytes_inside_immediates_are_not_calls_or_frame_setup() {
    let code = [
        0x55, 0x48, 0x89, 0xe5, 0x48, 0xb8, 0xe8, 0, 0, 0, 0, 0, 0, 0, // movabs rax,0xe8
        0xff, 0xd0,
    ];
    validate_calls(&code, &[16]).unwrap();
    assert_eq!(
        validate_calls(&code, &[11, 16]),
        Err(Error::ReturnPcDoesNotFollowCall { offset: 11 })
    );
    let fake_frame = [0x48, 0xb8, 0x55, 0x48, 0x89, 0xe5, 0, 0, 0, 0, 0xff, 0xd0];
    assert_eq!(
        validate_calls(&fake_frame, &[12]),
        Err(Error::MissingManagedFrameChain)
    );
    let reversed_frame = [0x48, 0x89, 0xe5, 0x55, 0xff, 0xd0];
    assert_eq!(
        validate_calls(&reversed_frame, &[6]),
        Err(Error::MissingManagedFrameChain)
    );
}
