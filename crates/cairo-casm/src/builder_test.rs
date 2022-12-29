use indoc::indoc;
use itertools::join;
use pretty_assertions::assert_eq;

use super::CasmBuilder;
use crate::builder::CasmBuildResult;
use crate::{casm_build_extend, res};

#[test]
fn test_ap_change_fixes() {
    let mut builder = CasmBuilder::default();
    let ap_at_7_mul_34 = builder.add_var(res!([ap + 7] * 34));
    let fp_at_minus_3 = builder.add_var(res!([fp - 3]));
    let imm5 = builder.add_var(res!(5));
    let ap_at_5 = builder.add_var(res!([ap + 5]));
    casm_build_extend! {builder,
        let ap_at_5_mul5 = ap_at_5 * imm5;
        ap += 2;
        let fp_at_minus_3_plus_ap_at_5 = fp_at_minus_3 + ap_at_5;
    };
    let CasmBuildResult { instructions, branches: [(state, awaiting_relocations)] } =
        builder.build(["Fallthrough"]);
    assert_eq!(state.get_adjusted(ap_at_7_mul_34), res!([ap + 5] * 34));
    assert_eq!(state.get_adjusted(fp_at_minus_3), res!([fp - 3]));
    assert_eq!(state.get_adjusted(ap_at_5), res!([ap + 3]));
    assert_eq!(state.get_adjusted(imm5), res!(5));
    assert_eq!(state.get_adjusted(ap_at_5_mul5), res!([ap + 3] * 5));
    assert_eq!(state.get_adjusted(fp_at_minus_3_plus_ap_at_5), res!([fp - 3] + [ap + 3]));
    assert_eq!(state.ap_change, 2);
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            ap += 2;
        "}
    );
    assert!(awaiting_relocations.is_empty());
}

#[test]
fn test_awaiting_relocations() {
    let mut builder = CasmBuilder::default();
    casm_build_extend! {builder,
        ap += 5;
        jump Target;
    };
    let CasmBuildResult { instructions, branches: [(state, awaiting_relocations)] } =
        builder.build(["Target"]);
    assert_eq!(state.ap_change, 5);
    assert_eq!(awaiting_relocations, [1]);
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            ap += 5;
            jmp rel 0;
        "}
    );
}

#[test]
fn test_noop_branch() {
    let mut builder = CasmBuilder::default();
    casm_build_extend! {builder,
        ap += 3;
        jump Target;
        Target:
    };
    let CasmBuildResult { instructions, branches: [(state, awaiting_relocations)] } =
        builder.build(["Fallthrough"]);
    assert!(awaiting_relocations.is_empty());
    assert_eq!(state.ap_change, 3);
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            ap += 3;
            jmp rel 2;
        "}
    );
}

#[test]
fn test_allocations() {
    let mut builder = CasmBuilder::default();
    casm_build_extend! {builder,
        tempvar a;
        tempvar b;
        tempvar c;
        assert a = b;
        assert b = c;
        assert c = a;
    };
    let CasmBuildResult { instructions, branches: [(state, awaiting_relocations)] } =
        builder.build(["Fallthrough"]);
    assert!(awaiting_relocations.is_empty());
    assert_eq!(state.ap_change, 3);
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            [ap + 0] = [ap + 1], ap++;
            [ap + 0] = [ap + 1], ap++;
            [ap + 0] = [ap + -2], ap++;
        "}
    );
}

#[test]
#[should_panic]
fn test_allocations_not_enough_commands() {
    let mut builder = CasmBuilder::default();
    casm_build_extend! {builder,
        tempvar a;
        tempvar b;
        tempvar c;
        assert a = b;
        assert b = c;
    };
    builder.build(["Fallthrough"]);
}

#[test]
fn test_aligned_branch_intersect() {
    let mut builder = CasmBuilder::default();
    let var = builder.add_var(res!([ap + 7]));
    casm_build_extend! {builder,
        tempvar _unused;
        jump X if var != 0;
        jump ONE_ALLOC;
        X:
        ONE_ALLOC:
    };
    let CasmBuildResult { instructions, branches: [(state, awaiting_relocations)] } =
        builder.build(["Fallthrough"]);
    assert!(awaiting_relocations.is_empty());
    assert_eq!(state.ap_change, 1);
    assert_eq!(state.allocated, 1);
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            jmp rel 4 if [ap + 7] != 0, ap++;
            jmp rel 2;
        "}
    );
}

#[test]
#[should_panic]
fn test_unaligned_branch_intersect() {
    let mut builder = CasmBuilder::default();
    let var = builder.add_var(res!([ap + 7]));
    casm_build_extend! {builder,
        jump X if var != 0;
        // A single tempvar in this branch.
        tempvar _unused;
        jump ONESIDED_ALLOC;
        // No allocs in this branch.
        X:
        // When the merge occurs here we will panic on a mismatch.
        ONESIDED_ALLOC:
    };
    builder.build(["Fallthrough"]);
}

#[test]
fn test_calculation_loop() {
    let mut builder = CasmBuilder::default();
    casm_build_extend! {builder,
        const one = 1;
        const ten = 10;
        tempvar a = one;
        tempvar n = ten;
        tempvar b = one;
        rescope{a = a, b = b, n = n, one = one};
        FIB:
        tempvar new_n = n - one;
        tempvar new_b = a + b;
        rescope{a = b, b = new_b, n = new_n, one = one};
        jump FIB if n != 0;
    };
    let CasmBuildResult { instructions, branches: [(state, awaiting_relocations)] } =
        builder.build(["Fallthrough"]);
    assert!(awaiting_relocations.is_empty());
    assert_eq!(state.get_adjusted(b), res!([ap - 1]));
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            [ap + 0] = 1, ap++;
            [ap + 0] = 10, ap++;
            [ap + 0] = 1, ap++;
            [ap + -2] = [ap + 0] + 1, ap++;
            [ap + 0] = [ap + -4] + [ap + -2], ap++;
            jmp rel -3 if [ap + -2] != 0;
        "}
    );
}

#[test]
fn test_call_ret() {
    let mut builder = CasmBuilder::default();
    casm_build_extend! {builder,
        const one = 1;
        const ten = 10;
        tempvar a = one;
        tempvar n = ten;
        tempvar b = one;
        call FIB;
        jump FT;
        FIB:
        tempvar new_a = b;
        tempvar new_n = n - one;
        tempvar new_b = a + b;
        jump REC_CALL if n != 0;
        rescope {};
        jump FIB_END;
        REC_CALL:
        call FIB;
        FIB_END:
        ret;
        FT:
    };
    let CasmBuildResult { instructions, branches: [(_, awaiting_relocations)] } =
        builder.build(["Fallthrough"]);
    assert!(awaiting_relocations.is_empty());
    assert_eq!(
        join(instructions.iter().map(|inst| format!("{inst};\n")), ""),
        indoc! {"
            [ap + 0] = 1, ap++;
            [ap + 0] = 10, ap++;
            [ap + 0] = 1, ap++;
            call rel 4;
            jmp rel 13;
            [ap + 0] = [fp + -3], ap++;
            [fp + -4] = [ap + 0] + 1, ap++;
            [ap + 0] = [fp + -5] + [fp + -3], ap++;
            jmp rel 4 if [fp + -4] != 0;
            jmp rel 4;
            call rel -8;
            ret;
        "}
    );
}