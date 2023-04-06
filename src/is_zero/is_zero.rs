use crate::is_zero::is_zero_gadget::{
    IsZeroChip,
    IsZeroConfig,
};

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value, layouter},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};

#[derive(Clone, Debug)]
struct ComposeConfig<F: FieldExt> {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Advice>,
    output: Column<Advice>,
    selector: Selector,
    a_equals_b: IsZeroConfig<F>,
}

#[derive(Clone, Debug)]
struct ComposeChip<F: FieldExt> {
    config: ComposeConfig<F>,
}

impl<F: FieldExt> ComposeChip<F> {
    fn construct(config: ComposeConfig<F>) -> Self {
        ComposeChip { config }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> ComposeConfig<F> {
        let selector = meta.selector();
        let a = meta.advice_column();
        let b = meta.advice_column();
        let c = meta.advice_column();
        let output = meta.advice_column();

        let is_zero_advice_column = meta.advice_column();

        let a_equals_b = IsZeroChip::configure(
            meta, 
            |meta| meta.query_selector(selector), 
            |meta| 
                meta.query_advice(a, Rotation::cur()) - meta.query_advice(b, Rotation::cur()), 
            is_zero_advice_column,
        );

        meta.create_gate("f(a, b, c) = a == b ? c : a - b", |meta| {
            let s = meta.query_selector(selector);
            let a = meta.query_advice(a, Rotation::cur());
            let b = meta.query_advice(b, Rotation::cur());
            let c = meta.query_advice(c, Rotation::cur());
            let output = meta.query_advice(output, Rotation::cur());

            vec![
                s.clone() * (a_equals_b.expr()) * (output.clone() - c),
                s * (Expression::Constant(F::one()) - a_equals_b.expr()) * (output - (a - b)),
            ]
        });
        ComposeConfig { a, b, c, output, selector, a_equals_b }
    }

    fn assign(
        &self,
        mut layouter: impl Layouter<F>,
        a: F,
        b: F,
        c: F,
    ) -> Result<AssignedCell<F, F>, Error> {
        let is_zero_chip = IsZeroChip::construct(self.config.a_equals_b.clone());

        layouter.assign_region(
            || "f(a, b, c) = a == b ? c : a - b",
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;
                region.assign_advice(|| "a", self.config.a, 0, || Value::known(a))?;
                region.assign_advice(|| "b", self.config.b, 0, || Value::known(b))?;
                region.assign_advice(|| "c", self.config.c, 0, || Value::known(c))?;
                is_zero_chip.assign(&mut region, 0, Value::known(a - b))?;

                let output = if a == b { c } else { a - b };
                region.assign_advice(|| "output", self.config.output, 0, || Value::known(output))
            }, 
        )
    }
}

#[derive(Default)]
struct ComposeCircuit<F> {
    a: F,
    b: F,
    c: F,
}

impl<F: FieldExt> Circuit<F> for ComposeCircuit<F> {
    type Config = ComposeConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        ComposeChip::configure(meta)
    }

    fn synthesize(&self, config: Self::Config, layouter: impl Layouter<F>) -> Result<(), Error> {
        let chip = ComposeChip::construct(config);

        chip.assign(layouter, self.a, self.b, self.c)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{
        dev::MockProver,
        pasta::Fp,
    };

    #[test]
    fn test_is_zero() {
        let k = 4;

        let circuit = ComposeCircuit {
            a: Fp::from(3),
            b: Fp::from(2),
            c: Fp::from(3),
        };

        let prover = MockProver::run(k, &circuit, vec![]).unwrap();
        prover.assert_satisfied();
    }
}