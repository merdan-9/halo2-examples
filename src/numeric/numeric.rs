use std::marker::PhantomData;
use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner, Value},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Fixed, Instance, Selector},
    poly::Rotation,
};

trait NumericInstructions<F: FieldExt>: Chip<F> {
    type Num;

    fn load_private(&self, layouter: impl Layouter<F>, value: Value<F>) -> Result<Self::Num, Error>;

    fn load_constant(&self, layouter: impl Layouter<F>, constant: F) -> Result<Self::Num, Error>;

    fn mul(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num,
    ) -> Result<Self::Num, Error>;

    fn expose_public(
        &self,
        layouter: impl Layouter<F>,
        num: Self::Num,
        row: usize,
    ) -> Result<(), Error>;
}

struct FieldChip<F: FieldExt> {
    config: FieldConfig,
    _marker: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct FieldConfig {
    advice: [Column<Advice>; 2],
    instance: Column<Instance>,
    selector: Selector,
}


impl<F: FieldExt> FieldChip<F> {
    fn construct(config: <Self as Chip<F>>::Config) -> Self {
        Self { 
            config, 
            _marker: PhantomData,
        }
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 2],
        instance: Column<Instance>,
        constant: Column<Fixed>,
    ) -> <Self as Chip<F>>::Config {
        meta.enable_equality(instance);
        meta.enable_constant(constant);
        for column in &advice {
            meta.enable_equality(*column);
        }
        let selector = meta.selector();

        meta.create_gate("mul", |meta| {
            let lhs = meta.query_advice(advice[0], Rotation::cur());
            let rhs = meta.query_advice(advice[1], Rotation::cur());
            let out = meta.query_advice(advice[0], Rotation::next());
            let selector = meta.query_selector(selector);
            vec![selector * (lhs * rhs - out)]
        });

        FieldConfig {
            advice,
            instance,
            selector,
        }
    }
}

impl<F: FieldExt> Chip<F> for FieldChip<F> {
    type Config = FieldConfig;
    type Loaded = ();
    
    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

#[derive(Clone)]
struct Number<F: FieldExt>(AssignedCell<F, F>);

impl<F: FieldExt> NumericInstructions<F> for FieldChip<F>  {
    type Num = Number<F>;

    fn load_private(&self, mut layouter: impl Layouter<F>, value: Value<F>) -> Result<Self::Num, Error> {
        let config = self.config();

        layouter.assign_region(
            || "load private",
            |mut region| {
                region
                    .assign_advice(|| "private input", config.advice[0], 0, || value)
                    .map(Number)
            },
        )
    }

    fn load_constant(&self, mut layouter: impl Layouter<F>, constant: F) -> Result<Self::Num, Error> {
        let config = self.config();

        layouter.assign_region(
            || "load constant", 
            |mut region| {
                region
                    .assign_advice_from_constant(|| "constant value", config.advice[0], 0, constant)
                    .map(Number)
            },
        )
    }

    fn mul(
        &self,
        mut layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num,
    ) -> Result<Self::Num, Error> {
        let config = self.config();

        layouter.assign_region(
            || "mul", 
            |mut region| {
                config.selector.enable(&mut region, 0)?;

                a.0.copy_advice(|| "lhs", &mut region, config.advice[0], 0)?;
                b.0.copy_advice(|| "rhs", &mut region, config.advice[1], 0)?;

                let value = a.0.value().copied() * b.0.value();

                region
                    .assign_advice(|| "lhs * rhs", config.advice[0], 1, || value)
                    .map(Number)
            },
        )
    }

    fn expose_public(
            &self,
            mut layouter: impl Layouter<F>,
            num: Self::Num,
            row: usize,
    ) -> Result<(), Error> {
        let config = self.config();

        layouter.constrain_instance(num.0.cell(), config.instance, row)
    }
}


#[derive(Default)]
struct MyCircuit<F: FieldExt> {
    constant: F,
    a: Value<F>,
    b: Value<F>,
}

impl<F: FieldExt> Circuit<F> for MyCircuit<F> {
    type Config = FieldConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = [meta.advice_column(), meta.advice_column()];
        let instance = meta.instance_column();
        let constant = meta.fixed_column();
        FieldChip::configure(meta, advice, instance, constant)
    }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let field_chip = FieldChip::<F>::construct(config);
        let a = field_chip.load_private(layouter.namespace(|| "load a"), self.a)?;
        let b = field_chip.load_private(layouter.namespace(|| "load b"), self.b)?;

        let constant = field_chip.load_constant(layouter.namespace(|| "load constant"), self.constant)?;
        let ab = field_chip.mul(layouter.namespace(|| "a * b"), a, b)?;
        let ab_sq = field_chip.mul(layouter.namespace(|| "ab * ab"), ab.clone(), ab)?;
        let c = field_chip.mul(layouter.namespace(|| "constant * ab_sq"), constant, ab_sq)?;

        field_chip.expose_public(layouter.namespace(|| "expose c"), c, 0)
        
    }
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        dev::MockProver, 
        pasta::Fp, 
        circuit::Value
    };
    use super::MyCircuit;
    
    #[test]
    fn numeric_instructions() {

        let k = 4;
        let constant = Fp::from(7);
        let a = Fp::from(2);
        let b = Fp::from(3);
        let c = constant * a.square() * b.square();

        let circuit = MyCircuit {
            constant,
            a: Value::known(a),
            b: Value::known(b),
        };

        let mut public_input = vec![c];

        let prover = MockProver::run(k, &circuit, vec![public_input.clone()]).unwrap();
        prover.assert_satisfied();
        
        public_input[0] += Fp::one();
        let prover = MockProver::run(k, &circuit, vec![public_input]).unwrap();
        assert!(prover.verify().is_err());
    }
}