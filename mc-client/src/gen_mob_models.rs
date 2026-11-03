// GERADO por scripts/gen_mob_models.py a partir dos construtores dos ModelXxx do Java 1.8.9. Não editar.
use crate::mob_rig::{GenCube, GenModel, GenPart};

pub const CREEPER: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "creeperArmor", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (16, 16), from: [-4.0, 0.0, -2.0], size: [8, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg1", pivot: [-2.0, 18.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [2.0, 18.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-2.0, 18.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [2.0, 18.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const CREEPER_CHARGED: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: 2.0, mirror: false }], children: &[] },
    GenPart { name: "creeperArmor", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: 2.5, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (16, 16), from: [-4.0, 0.0, -2.0], size: [8, 12, 4], inflate: 2.0, mirror: false }], children: &[] },
    GenPart { name: "leg1", pivot: [-2.0, 18.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 2.0, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [2.0, 18.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 2.0, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-2.0, 18.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 2.0, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [2.0, 18.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 2.0, mirror: false }], children: &[] },
] };
pub const PIG: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 12.0, -6.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -8.0], size: [8, 8, 8], inflate: 0.0, mirror: false }, GenCube { uv: (16, 16), from: [-2.0, 0.0, -9.0], size: [4, 3, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 11.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (28, 8), from: [-5.0, -10.0, -7.0], size: [10, 16, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg1", pivot: [-3.0, 18.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [3.0, 18.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-3.0, 18.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [3.0, 18.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const PIG_SADDLE: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 12.0, -6.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -8.0], size: [8, 8, 8], inflate: 0.5, mirror: false }, GenCube { uv: (16, 16), from: [-2.0, 0.0, -9.0], size: [4, 3, 1], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 11.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (28, 8), from: [-5.0, -10.0, -7.0], size: [10, 16, 8], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg1", pivot: [-3.0, 18.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [3.0, 18.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-3.0, 18.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [3.0, 18.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
] };
pub const COW: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "leg1", pivot: [-3.0, 12.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [3.0, 12.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-3.0, 12.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [3.0, 12.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "head", pivot: [0.0, 4.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -6.0], size: [8, 8, 6], inflate: 0.0, mirror: false }, GenCube { uv: (22, 0), from: [-5.0, -5.0, -4.0], size: [1, 3, 1], inflate: 0.0, mirror: false }, GenCube { uv: (22, 0), from: [4.0, -5.0, -4.0], size: [1, 3, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 5.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 4), from: [-6.0, -10.0, -7.0], size: [12, 18, 10], inflate: 0.0, mirror: false }, GenCube { uv: (52, 0), from: [-2.0, 2.0, -8.0], size: [4, 6, 1], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const SHEEP_BODY: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "leg1", pivot: [-3.0, 12.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [3.0, 12.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-3.0, 12.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [3.0, 12.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "head", pivot: [0.0, 6.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -4.0, -6.0], size: [6, 6, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 5.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (28, 8), from: [-4.0, -10.0, -7.0], size: [8, 16, 6], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const SHEEP_WOOL: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 6.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -4.0, -4.0], size: [6, 6, 6], inflate: 0.600000024, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 5.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (28, 8), from: [-4.0, -10.0, -7.0], size: [8, 16, 6], inflate: 1.75, mirror: false }], children: &[] },
    GenPart { name: "leg1", pivot: [-3.0, 12.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg2", pivot: [3.0, 12.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg3", pivot: [-3.0, 12.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "leg4", pivot: [3.0, 12.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 0.0, -2.0], size: [4, 6, 4], inflate: 0.5, mirror: false }], children: &[] },
] };
pub const CHICKEN: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 15.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-2.0, -6.0, -2.0], size: [4, 6, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bill", pivot: [0.0, 15.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (14, 0), from: [-2.0, -4.0, -4.0], size: [4, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "chin", pivot: [0.0, 15.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (14, 4), from: [-1.0, -2.0, -3.0], size: [2, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 16.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 9), from: [-3.0, -4.0, -3.0], size: [6, 8, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rightLeg", pivot: [-2.0, 19.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (26, 0), from: [-1.0, 0.0, -3.0], size: [3, 5, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leftLeg", pivot: [1.0, 19.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (26, 0), from: [-1.0, 0.0, -3.0], size: [3, 5, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rightWing", pivot: [-4.0, 13.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (24, 13), from: [0.0, 0.0, -3.0], size: [1, 4, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leftWing", pivot: [4.0, 13.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (24, 13), from: [-1.0, 0.0, -3.0], size: [1, 4, 6], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const SPIDER: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "spiderHead", pivot: [0.0, 15.0, -3.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 4), from: [-4.0, -4.0, -8.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderNeck", pivot: [0.0, 15.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -3.0, -3.0], size: [6, 6, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderBody", pivot: [0.0, 15.0, 9.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 12), from: [-5.0, -4.0, -6.0], size: [10, 8, 12], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg1", pivot: [-4.0, 15.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-15.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg2", pivot: [4.0, 15.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-1.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg3", pivot: [-4.0, 15.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-15.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg4", pivot: [4.0, 15.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-1.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg5", pivot: [-4.0, 15.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-15.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg6", pivot: [4.0, 15.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-1.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg7", pivot: [-4.0, 15.0, -1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-15.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spiderLeg8", pivot: [4.0, 15.0, -1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 0), from: [-1.0, -1.0, -1.0], size: [16, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const WOLF: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "wolfHeadMain", pivot: [-1.0, 13.5, -7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -3.0, -2.0], size: [6, 6, 4], inflate: 0.0, mirror: false }, GenCube { uv: (16, 14), from: [-3.0, -5.0, 0.0], size: [2, 2, 1], inflate: 0.0, mirror: false }, GenCube { uv: (16, 14), from: [1.0, -5.0, 0.0], size: [2, 2, 1], inflate: 0.0, mirror: false }, GenCube { uv: (0, 10), from: [-1.5, 0.0, -5.0], size: [3, 3, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfBody", pivot: [0.0, 14.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (18, 14), from: [-4.0, -2.0, -3.0], size: [6, 9, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfMane", pivot: [-1.0, 14.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (21, 0), from: [-4.0, -3.0, -3.0], size: [8, 6, 7], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfLeg1", pivot: [-2.5, 16.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 18), from: [-1.0, 0.0, -1.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfLeg2", pivot: [0.5, 16.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 18), from: [-1.0, 0.0, -1.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfLeg3", pivot: [-2.5, 16.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 18), from: [-1.0, 0.0, -1.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfLeg4", pivot: [0.5, 16.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 18), from: [-1.0, 0.0, -1.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wolfTail", pivot: [-1.0, 12.0, 8.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (9, 18), from: [-1.0, 0.0, -1.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const OCELOT: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "ocelotHead", pivot: [0.0, 15.0, -9.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-2.5, -2.0, -3.0], size: [5, 4, 5], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-1.5, 0.0, -4.0], size: [3, 2, 2], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-2.0, -3.0, 0.0], size: [1, 1, 2], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [1.0, -3.0, 0.0], size: [1, 1, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotBody", pivot: [0.0, 12.0, -10.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (20, 0), from: [-2.0, 3.0, -8.0], size: [4, 16, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotTail", pivot: [0.0, 15.0, 8.0], rot: [0.899999976, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 15), from: [-0.5, 0.0, 0.0], size: [1, 8, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotTail2", pivot: [0.0, 20.0, 14.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (4, 15), from: [-0.5, 0.0, 0.0], size: [1, 8, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotBackLeftLeg", pivot: [1.10000002, 18.0, 5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (8, 13), from: [-1.0, 0.0, 1.0], size: [2, 6, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotBackRightLeg", pivot: [-1.10000002, 18.0, 5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (8, 13), from: [-1.0, 0.0, 1.0], size: [2, 6, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotFrontLeftLeg", pivot: [1.20000005, 13.8000002, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (40, 0), from: [-1.0, 0.0, 0.0], size: [2, 10, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ocelotFrontRightLeg", pivot: [-1.20000005, 13.8000002, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (40, 0), from: [-1.0, 0.0, 0.0], size: [2, 10, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const RABBIT: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "rabbitLeftFoot", pivot: [3.0, 17.5, 3.70000005], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (26, 24), from: [-1.0, 5.5, -3.70000005], size: [2, 1, 7], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitRightFoot", pivot: [-3.0, 17.5, 3.70000005], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (8, 24), from: [-1.0, 5.5, -3.70000005], size: [2, 1, 7], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitLeftThigh", pivot: [3.0, 17.5, 3.70000005], rot: [-0.34906584, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (30, 15), from: [-1.0, 0.0, 0.0], size: [2, 4, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitRightThigh", pivot: [-3.0, 17.5, 3.70000005], rot: [-0.34906584, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (16, 15), from: [-1.0, 0.0, 0.0], size: [2, 4, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitBody", pivot: [0.0, 19.0, 8.0], rot: [-0.34906584, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -2.0, -10.0], size: [6, 5, 10], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitLeftArm", pivot: [3.0, 17.0, -1.0], rot: [-0.17453292, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (8, 15), from: [-1.0, 0.0, -1.0], size: [2, 7, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitRightArm", pivot: [-3.0, 17.0, -1.0], rot: [-0.17453292, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 15), from: [-1.0, 0.0, -1.0], size: [2, 7, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitHead", pivot: [0.0, 16.0, -1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 0), from: [-2.5, -4.0, -5.0], size: [5, 4, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitRightEar", pivot: [0.0, 16.0, -1.0], rot: [0.0, -0.261799395, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (52, 0), from: [-2.5, -9.0, -1.0], size: [2, 5, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitLeftEar", pivot: [0.0, 16.0, -1.0], rot: [0.0, 0.261799395, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (58, 0), from: [0.5, -9.0, -1.0], size: [2, 5, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitTail", pivot: [0.0, 20.0, 7.0], rot: [-0.3490659, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (52, 6), from: [-1.5, -1.5, 0.0], size: [3, 3, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rabbitNose", pivot: [0.0, 16.0, -1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 9), from: [-0.5, -2.5, -5.5], size: [1, 1, 1], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const SQUID: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "squidBody", pivot: [0.0, 8.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-6.0, -8.0, -6.0], size: [12, 16, 12], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_0", pivot: [5.0, 15.0, 0.0], rot: [0.0, 1.57079637, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_1", pivot: [3.53553391, 15.0, 3.53553391], rot: [0.0, 0.785398185, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_2", pivot: [3.061617e-16, 15.0, 5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_3", pivot: [-3.53553391, 15.0, 3.53553391], rot: [0.0, -0.785398185, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_4", pivot: [-5.0, 15.0, 6.123234e-16], rot: [0.0, -1.57079637, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_5", pivot: [-3.53553391, 15.0, -3.53553391], rot: [0.0, -2.3561945, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_6", pivot: [-9.18485047e-16, 15.0, -5.0], rot: [0.0, -3.14159274, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "squidTentacles_7", pivot: [3.53553391, 15.0, -3.53553391], rot: [0.0, -3.92699075, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (48, 0), from: [-1.0, 0.0, -1.0], size: [2, 18, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const BAT: GenModel = GenModel { tex: (64, 64), parts: &[
    GenPart { name: "batHead", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -3.0, -3.0], size: [6, 6, 6], inflate: 0.0, mirror: false }], children: &[6, 7] },
    GenPart { name: "batBody", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 16), from: [-3.0, 4.0, -3.0], size: [6, 12, 6], inflate: 0.0, mirror: false }, GenCube { uv: (0, 34), from: [-5.0, 16.0, 0.0], size: [10, 6, 1], inflate: 0.0, mirror: false }], children: &[2, 4] },
    GenPart { name: "batRightWing", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (42, 0), from: [-12.0, 1.0, 1.5], size: [10, 16, 1], inflate: 0.0, mirror: false }], children: &[3] },
    GenPart { name: "batOuterRightWing", pivot: [-12.0, 1.0, 1.5], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (24, 16), from: [-8.0, 1.0, 0.0], size: [8, 12, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "batLeftWing", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (42, 0), from: [2.0, 1.0, 1.5], size: [10, 16, 1], inflate: 0.0, mirror: true }], children: &[5] },
    GenPart { name: "batOuterLeftWing", pivot: [12.0, 1.0, 1.5], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (24, 16), from: [0.0, 1.0, 0.0], size: [8, 12, 1], inflate: 0.0, mirror: true }], children: &[] },
    GenPart { name: "anon_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (24, 0), from: [-4.0, -6.0, -2.0], size: [3, 4, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "anon_1", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (24, 0), from: [1.0, -6.0, -2.0], size: [3, 4, 1], inflate: 0.0, mirror: true }], children: &[] },
] };
pub const SLIME_INNER: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "slimeBodies", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-3.0, 17.0, -3.0], size: [6, 6, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "slimeRightEye", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 0), from: [-3.25, 18.0, -3.5], size: [2, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "slimeLeftEye", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 4), from: [1.25, 18.0, -3.5], size: [2, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "slimeMouth", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 8), from: [0.0, 21.0, -3.5], size: [1, 1, 1], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const SLIME_OUTER: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "slimeBodies", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, 16.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const MAGMA_CUBE: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "segments_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, 16.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_1", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 1), from: [-4.0, 17.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_2", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (24, 10), from: [-4.0, 18.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_3", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (24, 19), from: [-4.0, 19.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_4", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 4), from: [-4.0, 20.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_5", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 5), from: [-4.0, 21.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_6", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 6), from: [-4.0, 22.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "segments_7", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 7), from: [-4.0, 23.0, -4.0], size: [8, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "core", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-2.0, 18.0, -2.0], size: [4, 4, 4], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const GHAST: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "body", pivot: [0.0, 8.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-8.0, -8.0, -8.0], size: [16, 16, 16], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_0", pivot: [-3.75, 15.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_1", pivot: [1.25, 15.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 13, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_2", pivot: [6.25, 15.0, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_3", pivot: [-6.25, 15.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 11, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_4", pivot: [-1.25, 15.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 11, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_5", pivot: [3.75, 15.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 10, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_6", pivot: [-3.75, 15.0, 5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 12, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_7", pivot: [1.25, 15.0, 5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tentacles_8", pivot: [6.25, 15.0, 5.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.0, 0.0, -1.0], size: [2, 12, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const BLAZE: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "blazeSticks_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_1", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_2", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_3", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_4", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_5", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_6", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_7", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_8", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_9", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_10", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeSticks_11", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [0.0, 0.0, 0.0], size: [2, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "blazeHead", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const ENDERMAN: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "bipedHead", pivot: [0.0, -14.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedHeadwear", pivot: [0.0, -14.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: -0.5, mirror: false }], children: &[] },
    GenPart { name: "bipedBody", pivot: [0.0, -14.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 16), from: [-4.0, 0.0, -2.0], size: [8, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedRightArm", pivot: [-3.0, -12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (56, 0), from: [-1.0, -2.0, -1.0], size: [2, 30, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedLeftArm", pivot: [5.0, -12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (56, 0), from: [-1.0, -2.0, -1.0], size: [2, 30, 2], inflate: 0.0, mirror: true }], children: &[] },
    GenPart { name: "bipedRightLeg", pivot: [-2.0, -2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (56, 0), from: [-1.0, 0.0, -1.0], size: [2, 30, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedLeftLeg", pivot: [2.0, -2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (56, 0), from: [-1.0, 0.0, -1.0], size: [2, 30, 2], inflate: 0.0, mirror: true }], children: &[] },
] };
pub const ENDERMITE: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "field_178713_d_0", pivot: [0.0, 21.0, -3.5], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-2.0, 0.0, -1.0], size: [4, 3, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_178713_d_1", pivot: [0.0, 20.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 5), from: [-3.0, 0.0, -2.5], size: [6, 4, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_178713_d_2", pivot: [0.0, 21.0, 3.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 14), from: [-1.5, 0.0, -0.5], size: [3, 3, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_178713_d_3", pivot: [0.0, 22.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 18), from: [-0.5, 0.0, -0.5], size: [1, 2, 1], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const SILVERFISH: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "silverfishBodyParts_0", pivot: [0.0, 22.0, -3.5], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-1.5, 0.0, -1.0], size: [3, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishBodyParts_1", pivot: [0.0, 21.0, -1.5], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 4), from: [-2.0, 0.0, -1.0], size: [4, 3, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishBodyParts_2", pivot: [0.0, 20.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 9), from: [-3.0, 0.0, -1.5], size: [6, 4, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishBodyParts_3", pivot: [0.0, 21.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-1.5, 0.0, -1.5], size: [3, 3, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishBodyParts_4", pivot: [0.0, 22.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 22), from: [-1.0, 0.0, -1.5], size: [2, 2, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishBodyParts_5", pivot: [0.0, 23.0, 9.5], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (11, 0), from: [-1.0, 0.0, -1.0], size: [2, 1, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishBodyParts_6", pivot: [0.0, 23.0, 11.5], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (13, 4), from: [-0.5, 0.0, -1.0], size: [1, 1, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishWings_0", pivot: [0.0, 16.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (20, 0), from: [-5.0, 0.0, -1.5], size: [10, 8, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishWings_1", pivot: [0.0, 20.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (20, 11), from: [-3.0, 0.0, -1.5], size: [6, 4, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "silverfishWings_2", pivot: [0.0, 19.0, -1.5], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (20, 18), from: [-3.0, 0.0, -1.5], size: [6, 5, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const WITCH: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "field_82901_h", pivot: [0.0, -2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (0, 0), from: [0.0, 3.0, -6.75], size: [1, 1, 1], inflate: -0.25, mirror: false }], children: &[] },
    GenPart { name: "villagerHead", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -10.0, -4.0], size: [8, 10, 8], inflate: 0.0, mirror: false }], children: &[2, 7] },
    GenPart { name: "villagerNose", pivot: [0.0, -2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (24, 0), from: [-1.0, -1.0, -6.0], size: [2, 4, 2], inflate: 0.0, mirror: false }], children: &[0] },
    GenPart { name: "villagerBody", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (16, 20), from: [-4.0, 0.0, -3.0], size: [8, 12, 6], inflate: 0.0, mirror: false }, GenCube { uv: (0, 38), from: [-4.0, 0.0, -3.0], size: [8, 18, 6], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "villagerArms", pivot: [0.0, 2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (44, 22), from: [-8.0, -2.0, -2.0], size: [4, 8, 4], inflate: 0.0, mirror: false }, GenCube { uv: (44, 22), from: [4.0, -2.0, -2.0], size: [4, 8, 4], inflate: 0.0, mirror: false }, GenCube { uv: (40, 38), from: [-4.0, 2.0, -2.0], size: [8, 4, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rightVillagerLeg", pivot: [-2.0, 12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (0, 22), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "leftVillagerLeg", pivot: [2.0, 12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (0, 22), from: [-2.0, 0.0, -2.0], size: [4, 12, 4], inflate: 0.0, mirror: true }], children: &[] },
    GenPart { name: "witchHat", pivot: [-5.0, -10.03125, -5.0], rot: [0.0, 0.0, 0.0], tex: (64, 128), cubes: &[GenCube { uv: (0, 64), from: [0.0, 0.0, 0.0], size: [10, 2, 10], inflate: 0.0, mirror: false }], children: &[8] },
    GenPart { name: "anon_0", pivot: [1.75, -4.0, 2.0], rot: [-0.052359879, 0.0, 0.0261799395], tex: (64, 128), cubes: &[GenCube { uv: (0, 76), from: [0.0, 0.0, 0.0], size: [7, 4, 7], inflate: 0.0, mirror: false }], children: &[9] },
    GenPart { name: "anon_1", pivot: [1.75, -4.0, 2.0], rot: [-0.104719758, 0.0, 0.052359879], tex: (64, 128), cubes: &[GenCube { uv: (0, 87), from: [0.0, 0.0, 0.0], size: [4, 4, 4], inflate: 0.0, mirror: false }], children: &[10] },
    GenPart { name: "anon_2", pivot: [1.75, -2.0, 2.0], rot: [-0.209439516, 0.0, 0.104719758], tex: (64, 128), cubes: &[GenCube { uv: (0, 95), from: [0.0, 0.0, 0.0], size: [1, 2, 1], inflate: 0.25, mirror: false }], children: &[] },
] };
pub const IRON_GOLEM: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "ironGolemHead", pivot: [0.0, -7.0, -2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -12.0, -5.5], size: [8, 10, 8], inflate: 0.0, mirror: false }, GenCube { uv: (24, 0), from: [-1.0, -5.0, -7.5], size: [2, 4, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ironGolemBody", pivot: [0.0, -7.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 40), from: [-9.0, -2.0, -6.0], size: [18, 12, 11], inflate: 0.0, mirror: false }, GenCube { uv: (0, 70), from: [-4.5, 10.0, -3.0], size: [9, 5, 6], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "ironGolemRightArm", pivot: [0.0, -7.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (60, 21), from: [-13.0, -2.5, -3.0], size: [4, 30, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ironGolemLeftArm", pivot: [0.0, -7.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (60, 58), from: [9.0, -2.5, -3.0], size: [4, 30, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ironGolemLeftLeg", pivot: [-4.0, 11.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (37, 0), from: [-3.5, -3.0, -3.0], size: [6, 16, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "ironGolemRightLeg", pivot: [5.0, 11.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (60, 0), from: [-3.5, -3.0, -3.0], size: [6, 16, 5], inflate: 0.0, mirror: true }], children: &[] },
] };
pub const SNOWMAN: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "head", pivot: [0.0, 4.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: -0.5, mirror: false }], children: &[] },
    GenPart { name: "rightHand", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-1.0, 0.0, -1.0], size: [12, 2, 2], inflate: -0.5, mirror: false }], children: &[] },
    GenPart { name: "leftHand", pivot: [0.0, 6.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-1.0, 0.0, -1.0], size: [12, 2, 2], inflate: -0.5, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 13.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 16), from: [-5.0, -10.0, -5.0], size: [10, 10, 10], inflate: -0.5, mirror: false }], children: &[] },
    GenPart { name: "bottomBody", pivot: [0.0, 24.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 36), from: [-6.0, -12.0, -6.0], size: [12, 12, 12], inflate: -0.5, mirror: false }], children: &[] },
] };
pub const GUARDIAN: GenModel = GenModel { tex: (64, 64), parts: &[
    GenPart { name: "guardianBody", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-6.0, 10.0, -8.0], size: [12, 12, 16], inflate: 0.0, mirror: false }, GenCube { uv: (0, 28), from: [-8.0, 10.0, -6.0], size: [2, 12, 12], inflate: 0.0, mirror: false }, GenCube { uv: (0, 28), from: [6.0, 10.0, -6.0], size: [2, 12, 12], inflate: 0.0, mirror: true }, GenCube { uv: (16, 40), from: [-6.0, 8.0, -6.0], size: [12, 2, 12], inflate: 0.0, mirror: false }, GenCube { uv: (16, 40), from: [-6.0, 22.0, -6.0], size: [12, 2, 12], inflate: 0.0, mirror: false }], children: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] },
    GenPart { name: "guardianSpines_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_1", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_2", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_3", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_4", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_5", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_6", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_7", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_8", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_9", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_10", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianSpines_11", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -4.5, -1.0], size: [2, 9, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianEye", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (8, 0), from: [-1.0, 15.0, 0.0], size: [2, 2, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "guardianTail_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (40, 0), from: [-2.0, 14.0, 7.0], size: [4, 4, 8], inflate: 0.0, mirror: false }], children: &[15] },
    GenPart { name: "guardianTail_1", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 54), from: [0.0, 14.0, 0.0], size: [3, 3, 7], inflate: 0.0, mirror: false }], children: &[16] },
    GenPart { name: "guardianTail_2", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (41, 32), from: [0.0, 14.0, 0.0], size: [2, 2, 6], inflate: 0.0, mirror: false }, GenCube { uv: (25, 19), from: [1.0, 10.5, 3.0], size: [1, 9, 9], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const WITHER: GenModel = GenModel { tex: (64, 64), parts: &[
    GenPart { name: "field_82905_a_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 16), from: [-10.0, 3.9000001, -0.5], size: [20, 3, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_82905_a_1", pivot: [-2.0, 6.9000001, -0.5], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 22), from: [0.0, 0.0, 0.0], size: [3, 10, 3], inflate: 0.0, mirror: false }, GenCube { uv: (24, 22), from: [-4.0, 1.5, 0.5], size: [11, 2, 2], inflate: 0.0, mirror: false }, GenCube { uv: (24, 22), from: [-4.0, 4.0, 0.5], size: [11, 2, 2], inflate: 0.0, mirror: false }, GenCube { uv: (24, 22), from: [-4.0, 6.5, 0.5], size: [11, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_82905_a_2", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (12, 22), from: [0.0, 0.0, 0.0], size: [3, 6, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_82904_b_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_82904_b_1", pivot: [-8.0, 4.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -4.0, -4.0], size: [6, 6, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_82904_b_2", pivot: [10.0, 4.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -4.0, -4.0], size: [6, 6, 6], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const WITHER_ARMOR: GenModel = GenModel { tex: (64, 64), parts: &[
    GenPart { name: "field_82905_a_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 16), from: [-10.0, 3.9000001, -0.5], size: [20, 3, 3], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "field_82905_a_1", pivot: [-2.0, 6.9000001, -0.5], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 22), from: [0.0, 0.0, 0.0], size: [3, 10, 3], inflate: 0.5, mirror: false }, GenCube { uv: (24, 22), from: [-4.0, 1.5, 0.5], size: [11, 2, 2], inflate: 0.5, mirror: false }, GenCube { uv: (24, 22), from: [-4.0, 4.0, 0.5], size: [11, 2, 2], inflate: 0.5, mirror: false }, GenCube { uv: (24, 22), from: [-4.0, 6.5, 0.5], size: [11, 2, 2], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "field_82905_a_2", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (12, 22), from: [0.0, 0.0, 0.0], size: [3, 6, 3], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "field_82904_b_0", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "field_82904_b_1", pivot: [-8.0, 4.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -4.0, -4.0], size: [6, 6, 6], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "field_82904_b_2", pivot: [10.0, 4.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -4.0, -4.0], size: [6, 6, 6], inflate: 0.5, mirror: false }], children: &[] },
] };
pub const DRAGON: GenModel = GenModel { tex: (256, 256), parts: &[
    GenPart { name: "head", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-6.0, -1.0, -24.0], size: [12, 5, 16], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-8.0, -8.0, -10.0], size: [16, 16, 16], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-5.0, -12.0, -4.0], size: [2, 4, 6], inflate: 0.0, mirror: true }, GenCube { uv: (0, 0), from: [-5.0, -3.0, -22.0], size: [2, 2, 4], inflate: 0.0, mirror: true }, GenCube { uv: (0, 0), from: [3.0, -12.0, -4.0], size: [2, 4, 6], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [3.0, -3.0, -22.0], size: [2, 2, 4], inflate: 0.0, mirror: false }], children: &[1] },
    GenPart { name: "jaw", pivot: [0.0, 4.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-6.0, 0.0, -16.0], size: [12, 4, 16], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "spine", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-5.0, -5.0, -5.0], size: [10, 10, 10], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-1.0, -9.0, -3.0], size: [2, 4, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "body", pivot: [0.0, 4.0, 8.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-12.0, 0.0, -16.0], size: [24, 24, 64], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-1.0, -6.0, -10.0], size: [2, 6, 12], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-1.0, -6.0, 10.0], size: [2, 6, 12], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-1.0, -6.0, 30.0], size: [2, 6, 12], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "wing", pivot: [-12.0, 5.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-56.0, -4.0, -4.0], size: [56, 8, 8], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-56.0, 0.0, 2.0], size: [56, 0, 56], inflate: 0.0, mirror: false }], children: &[5] },
    GenPart { name: "wingTip", pivot: [-56.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-56.0, -2.0, -2.0], size: [56, 4, 4], inflate: 0.0, mirror: false }, GenCube { uv: (0, 0), from: [-56.0, 0.0, 2.0], size: [56, 0, 56], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontLeg", pivot: [-12.0, 20.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -4.0], size: [8, 24, 8], inflate: 0.0, mirror: false }], children: &[7] },
    GenPart { name: "frontLegTip", pivot: [0.0, 20.0, -1.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -1.0, -3.0], size: [6, 24, 6], inflate: 0.0, mirror: false }], children: &[8] },
    GenPart { name: "frontFoot", pivot: [0.0, 23.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-4.0, 0.0, -12.0], size: [8, 4, 16], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "rearLeg", pivot: [-16.0, 16.0, 42.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-8.0, -4.0, -8.0], size: [16, 32, 16], inflate: 0.0, mirror: false }], children: &[10] },
    GenPart { name: "rearLegTip", pivot: [0.0, 32.0, -4.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-6.0, -2.0, 0.0], size: [12, 32, 12], inflate: 0.0, mirror: false }], children: &[11] },
    GenPart { name: "rearFoot", pivot: [0.0, 31.0, 4.0], rot: [0.0, 0.0, 0.0], tex: (256, 256), cubes: &[GenCube { uv: (0, 0), from: [-9.0, 0.0, -20.0], size: [18, 6, 24], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const HORSE: GenModel = GenModel { tex: (128, 128), parts: &[
    GenPart { name: "body", pivot: [0.0, 11.0, 9.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 34), from: [-5.0, -8.0, -19.0], size: [10, 10, 24], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tailBase", pivot: [0.0, 3.0, 14.0], rot: [-1.13446403, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (44, 0), from: [-1.0, -1.0, 0.0], size: [2, 2, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tailMiddle", pivot: [0.0, 3.0, 14.0], rot: [-1.13446403, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (38, 7), from: [-1.5, -2.0, 3.0], size: [3, 4, 7], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "tailTip", pivot: [0.0, 3.0, 14.0], rot: [-1.40215003, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (24, 3), from: [-1.5, -4.5, 9.0], size: [3, 4, 7], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "backLeftLeg", pivot: [4.0, 9.0, 11.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (78, 29), from: [-2.5, -2.0, -2.5], size: [4, 9, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "backLeftShin", pivot: [4.0, 16.0, 11.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (78, 43), from: [-2.0, 0.0, -1.5], size: [3, 5, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "backLeftHoof", pivot: [4.0, 16.0, 11.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (78, 51), from: [-2.5, 5.0999999, -2.0], size: [4, 3, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "backRightLeg", pivot: [-4.0, 9.0, 11.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (96, 29), from: [-1.5, -2.0, -2.5], size: [4, 9, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "backRightShin", pivot: [-4.0, 16.0, 11.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (96, 43), from: [-1.0, 0.0, -1.5], size: [3, 5, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "backRightHoof", pivot: [-4.0, 16.0, 11.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (96, 51), from: [-1.5, 5.0999999, -2.0], size: [4, 3, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontLeftLeg", pivot: [4.0, 9.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (44, 29), from: [-1.89999998, -1.0, -2.0999999], size: [3, 8, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontLeftShin", pivot: [4.0, 16.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (44, 41), from: [-1.89999998, 0.0, -1.60000002], size: [3, 5, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontLeftHoof", pivot: [4.0, 16.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (44, 51), from: [-2.4000001, 5.0999999, -2.0999999], size: [4, 3, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontRightLeg", pivot: [-4.0, 9.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (60, 29), from: [-1.10000002, -1.0, -2.0999999], size: [3, 8, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontRightShin", pivot: [-4.0, 16.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (60, 41), from: [-1.10000002, 0.0, -1.60000002], size: [3, 5, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "frontRightHoof", pivot: [-4.0, 16.0, -8.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (60, 51), from: [-1.60000002, 5.0999999, -2.0999999], size: [4, 3, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "head", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 0), from: [-2.5, -10.0, -1.5], size: [5, 5, 7], inflate: 0.0, mirror: false }], children: &[17, 18] },
    GenPart { name: "field_178711_b", pivot: [0.0, 3.95000005, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (24, 18), from: [-2.0, -10.0, -7.0], size: [4, 3, 6], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "field_178712_c", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (24, 27), from: [-2.0, -7.0, -6.5], size: [4, 2, 5], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseLeftEar", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 0), from: [0.449999988, -12.0, 4.0], size: [2, 3, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseRightEar", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 0), from: [-2.45000005, -12.0, 4.0], size: [2, 3, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "muleLeftEar", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.261799395], tex: (128, 128), cubes: &[GenCube { uv: (0, 12), from: [-2.0, -16.0, 4.0], size: [2, 7, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "muleRightEar", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, -0.261799395], tex: (128, 128), cubes: &[GenCube { uv: (0, 12), from: [0.0, -16.0, 4.0], size: [2, 7, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "neck", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 12), from: [-2.04999995, -9.80000019, -2.0], size: [4, 14, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "muleLeftChest", pivot: [-7.5, 3.0, 10.0], rot: [0.0, 1.57079637, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 34), from: [-3.0, 0.0, 0.0], size: [8, 8, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "muleRightChest", pivot: [4.5, 3.0, 10.0], rot: [0.0, 1.57079637, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (0, 47), from: [-3.0, 0.0, 0.0], size: [8, 8, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseSaddleBottom", pivot: [0.0, 2.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (80, 0), from: [-5.0, 0.0, -3.0], size: [10, 1, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseSaddleFront", pivot: [0.0, 2.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (106, 9), from: [-1.5, -1.0, -3.0], size: [3, 1, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseSaddleBack", pivot: [0.0, 2.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (80, 9), from: [-4.0, -1.0, 3.0], size: [8, 1, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseLeftSaddleMetal", pivot: [5.0, 3.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (74, 0), from: [-0.5, 6.0, -1.0], size: [1, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseLeftSaddleRope", pivot: [5.0, 3.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (70, 0), from: [-0.5, 0.0, -0.5], size: [1, 6, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseRightSaddleMetal", pivot: [-5.0, 3.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (74, 4), from: [-0.5, 6.0, -1.0], size: [1, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseRightSaddleRope", pivot: [-5.0, 3.0, 2.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (80, 0), from: [-0.5, 0.0, -0.5], size: [1, 6, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseLeftFaceMetal", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (74, 13), from: [1.5, -8.0, -4.0], size: [1, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseRightFaceMetal", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (74, 13), from: [-2.5, -8.0, -4.0], size: [1, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseLeftRein", pivot: [0.0, 4.0, -10.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (44, 10), from: [2.5999999, -6.0, -6.0], size: [0, 3, 16], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseRightRein", pivot: [0.0, 4.0, -10.0], rot: [0.0, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (44, 5), from: [-2.5999999, -6.0, -6.0], size: [0, 3, 16], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "mane", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (58, 0), from: [-1.0, -11.5, 5.0], size: [2, 16, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "horseFaceRopes", pivot: [0.0, 4.0, -10.0], rot: [0.52359879, 0.0, 0.0], tex: (128, 128), cubes: &[GenCube { uv: (80, 12), from: [-2.5, -10.1000004, -7.0], size: [5, 5, 12], inflate: 0.200000003, mirror: false }], children: &[] },
] };
pub const BOAT: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "boatSides_0", pivot: [0.0, 4.0, 0.0], rot: [1.57079637, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 8), from: [-12.0, -8.0, -3.0], size: [24, 16, 4], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "boatSides_1", pivot: [-11.0, 4.0, 0.0], rot: [0.0, 4.71238899, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-10.0, -7.0, -1.0], size: [20, 6, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "boatSides_2", pivot: [11.0, 4.0, 0.0], rot: [0.0, 1.57079637, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-10.0, -7.0, -1.0], size: [20, 6, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "boatSides_3", pivot: [0.0, 4.0, -9.0], rot: [0.0, 3.14159274, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-10.0, -7.0, -1.0], size: [20, 6, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "boatSides_4", pivot: [0.0, 4.0, 9.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-10.0, -7.0, -1.0], size: [20, 6, 2], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const MINECART: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "sideModels_0", pivot: [0.0, 4.0, 0.0], rot: [1.57079637, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 10), from: [-10.0, -8.0, -1.0], size: [20, 16, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "sideModels_1", pivot: [-9.0, 4.0, 0.0], rot: [0.0, 4.71238899, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-8.0, -9.0, -1.0], size: [16, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "sideModels_2", pivot: [9.0, 4.0, 0.0], rot: [0.0, 1.57079637, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-8.0, -9.0, -1.0], size: [16, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "sideModels_3", pivot: [0.0, 4.0, -7.0], rot: [0.0, 3.14159274, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-8.0, -9.0, -1.0], size: [16, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "sideModels_4", pivot: [0.0, 4.0, 7.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-8.0, -9.0, -1.0], size: [16, 8, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "sideModels_5", pivot: [0.0, 4.0, 0.0], rot: [-1.57079637, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (44, 10), from: [-9.0, -7.0, -1.0], size: [18, 14, 1], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const ENDER_CRYSTAL: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "glass", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "cube", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "base", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 16), from: [-6.0, 0.0, -6.0], size: [12, 4, 12], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const ENDER_CRYSTAL_NOBASE: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "glass", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "cube", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -4.0, -4.0], size: [8, 8, 8], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const ARMOR_STAND: GenModel = GenModel { tex: (64, 64), parts: &[
    GenPart { name: "bipedHeadwear", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 0), from: [-4.0, -8.0, -4.0], size: [8, 8, 8], inflate: 0.5, mirror: false }], children: &[] },
    GenPart { name: "bipedHead", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 0), from: [-1.0, -7.0, -1.0], size: [2, 7, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedBody", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 26), from: [-6.0, 0.0, -1.5], size: [12, 3, 3], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedRightArm", pivot: [-5.0, 2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (24, 0), from: [-2.0, -2.0, -1.0], size: [2, 12, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedLeftArm", pivot: [5.0, 2.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (32, 16), from: [0.0, -2.0, -1.0], size: [2, 12, 2], inflate: 0.0, mirror: true }], children: &[] },
    GenPart { name: "bipedRightLeg", pivot: [-1.89999998, 12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (8, 0), from: [-1.0, 0.0, -1.0], size: [2, 11, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bipedLeftLeg", pivot: [1.89999998, 12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (40, 16), from: [-1.0, 0.0, -1.0], size: [2, 11, 2], inflate: 0.0, mirror: true }], children: &[] },
    GenPart { name: "standRightSide", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (16, 0), from: [-3.0, 3.0, -1.0], size: [2, 7, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "standLeftSide", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (48, 16), from: [1.0, 3.0, -1.0], size: [2, 7, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "standWaist", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 48), from: [-4.0, 10.0, -1.0], size: [8, 2, 2], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "standBase", pivot: [0.0, 12.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 64), cubes: &[GenCube { uv: (0, 32), from: [-6.0, 11.0, -6.0], size: [12, 1, 12], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const LEASH_KNOT: GenModel = GenModel { tex: (32, 32), parts: &[
    GenPart { name: "field_110723_a", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (32, 32), cubes: &[GenCube { uv: (0, 0), from: [-3.0, -6.0, -3.0], size: [6, 8, 6], inflate: 0.0, mirror: false }], children: &[] },
] };
pub const BOOK: GenModel = GenModel { tex: (64, 32), parts: &[
    GenPart { name: "coverRight", pivot: [0.0, 0.0, -1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 0), from: [-6.0, -5.0, 0.0], size: [6, 10, 0], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "coverLeft", pivot: [0.0, 0.0, 1.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (16, 0), from: [0.0, -5.0, 0.0], size: [6, 10, 0], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "pagesRight", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (0, 10), from: [0.0, -4.0, -0.99000001], size: [5, 8, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "pagesLeft", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (12, 10), from: [0.0, -4.0, -0.00999999978], size: [5, 8, 1], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "flippingPageRight", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (24, 10), from: [0.0, -4.0, 0.0], size: [5, 8, 0], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "flippingPageLeft", pivot: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (24, 10), from: [0.0, -4.0, 0.0], size: [5, 8, 0], inflate: 0.0, mirror: false }], children: &[] },
    GenPart { name: "bookSpine", pivot: [0.0, 0.0, 0.0], rot: [0.0, 1.57079637, 0.0], tex: (64, 32), cubes: &[GenCube { uv: (12, 0), from: [-1.0, -5.0, 0.0], size: [2, 10, 0], inflate: 0.0, mirror: false }], children: &[] },
] };

/// `EntitySheep.DYE_TO_RGB` pela metadata do `EnumDyeColor`.
pub const SHEEP_COLORS: [[f32; 3]; 16] = [[1.0, 1.0, 1.0], [0.850000024, 0.5, 0.200000003], [0.699999988, 0.300000012, 0.850000024], [0.400000006, 0.600000024, 0.850000024], [0.899999976, 0.899999976, 0.200000003], [0.5, 0.800000012, 0.100000001], [0.949999988, 0.5, 0.649999976], [0.300000012, 0.300000012, 0.300000012], [0.600000024, 0.600000024, 0.600000024], [0.300000012, 0.5, 0.600000024], [0.5, 0.25, 0.699999988], [0.200000003, 0.300000012, 0.699999988], [0.400000006, 0.300000012, 0.200000003], [0.400000006, 0.5, 0.200000003], [0.600000024, 0.200000003, 0.200000003], [0.100000001, 0.100000001, 0.100000001]];
