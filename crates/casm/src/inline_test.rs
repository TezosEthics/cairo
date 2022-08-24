use indoc::indoc;

use crate::instructions::*;
use crate::operand::*;
use crate::{casm, deref};

#[test]
fn test_assert() {
    let x = ResOperand::Immediate(ImmediateOperand { value: 1 });
    let y = deref!([FP + 5]);

    let ctx = casm! {
        assert([FP - 5] = x), ap++;
        assert([FP - 5] = ([AP + 1] + [FP - 5])), ap++;
        assert([FP + 5] = ([AP + 1] + 2));
        assert([AP] = ([AP + 1] * [FP - 5]));
        assert([FP - 5] = ([AP + 1] * 2));
        assert([FP - 5] = ([AP + 1] * y));
        assert([FP - 5] = 1), ap++;
        assert([FP - 5] = [AP + 1]);
        call(abs 5), ap++;
        call(rel y), ap++;
    };

    let code = ctx.instructions.iter().map(Instruction::to_string).collect::<Vec<_>>().join("\n");
    assert_eq!(
        code,
        indoc! {"
            [fp + -5] = 1, ap++
            [fp + -5] = [ap + 1] + [fp + -5], ap++
            [fp + 5] = [ap + 1] + 2
            [ap + 0] = [ap + 1] + [fp + -5]
            [fp + -5] = [ap + 1] + 2
            [fp + -5] = [ap + 1] + [fp + 5]
            [fp + -5] = 1, ap++
            [fp + -5] = [ap + 1]
            call abs 5, ap++
            call rel [fp + 5], ap++"}
    );
}
