//! Dados de entidades que só o HUD lê: absorção e ar do jogador, vida e vida máxima da
//! montaria e dos chefes (`BossStatus`). Alimentado pelos mesmos pacotes de entidade que o
//! `entities.rs` recebe (0x0F, 0x1C, 0x13) e pelo 0x20.

use std::collections::HashMap;

use mc_protocol::Reader;

use crate::entity_net::{EntityPacket, MetaValue, Metadata};

/// Tipos de mob (id do 0x0F) que chamam `BossStatus.setBossStatus` ao serem desenhados.
pub const ENDER_DRAGON: u8 = 63;
pub const WITHER: u8 = 64;

/// Vida máxima padrão dos chefes (`applyEntityAttributes` de `EntityDragon` e `EntityWither`);
/// as demais entidades usam o valor do 0x20, ou 20 (`SharedMonsterAttributes.maxHealth`).
pub fn default_max_health(kind: u8) -> f32 {
    match kind {
        ENDER_DRAGON => 200.0,
        WITHER => 300.0,
        _ => 20.0,
    }
}

#[derive(Default, Debug)]
pub struct Track {
    /// Metadado 17 do jogador local (`getAbsorptionAmount`).
    pub absorption: f32,
    /// Metadado 1 (ar restante, 300 = cheio).
    pub air: i32,
    /// Metadado 0 do jogador local (bit 1: em chamas, `getFlag(0)`).
    pub flags: i8,
    /// Metadado 18 do jogador local (`EntityPlayer.getScore`, mostrado na tela de morte).
    pub score: i32,
    pub health: HashMap<i32, f32>,
    pub max_health: HashMap<i32, f32>,
    /// Entidades que são chefes e o tipo delas.
    pub bosses: HashMap<i32, u8>,
    /// Nome personalizado (metadado 2) de chefes.
    pub names: HashMap<i32, String>,
    /// Tipo de cada mob (0x0F) e as flags do cavalo (metadado 16, bit 4 = selado).
    pub kinds: HashMap<i32, u8>,
    pub horse_flags: HashMap<i32, i32>,
}

/// Tipo do cavalo no 0x0F.
pub const HORSE: u8 = 100;

impl Track {
    pub fn new() -> Self {
        Self { air: 300, ..Default::default() }
    }

    fn meta(&mut self, id: i32, own: Option<i32>, meta: &Metadata) {
        for (idx, v) in meta {
            match (idx, v) {
                (6, MetaValue::Float(f)) => {
                    self.health.insert(id, *f);
                }
                (17, MetaValue::Float(f)) if Some(id) == own => self.absorption = *f,
                (1, MetaValue::Short(a)) if Some(id) == own => self.air = *a as i32,
                (18, MetaValue::Int(n)) if Some(id) == own => self.score = *n,
                (0, MetaValue::Byte(b)) if Some(id) == own => self.flags = *b,
                (16, MetaValue::Int(f)) if self.kinds.get(&id) == Some(&HORSE) => {
                    self.horse_flags.insert(id, *f);
                }
                (2, MetaValue::Str(s)) if self.bosses.contains_key(&id) => {
                    self.names.insert(id, s.clone());
                }
                _ => {}
            }
        }
    }

    pub fn packet(&mut self, p: &EntityPacket, own: Option<i32>) {
        match p {
            EntityPacket::SpawnMob { id, kind, meta, .. } => {
                self.kinds.insert(*id, *kind);
                if matches!(*kind, ENDER_DRAGON | WITHER) {
                    self.bosses.insert(*id, *kind);
                }
                self.max_health.entry(*id).or_insert_with(|| default_max_health(*kind));
                self.meta(*id, own, meta);
            }
            EntityPacket::SpawnPlayer { id, meta, .. } => self.meta(*id, own, meta),
            EntityPacket::Metadata { id, meta } => self.meta(*id, own, meta),
            EntityPacket::Destroy(ids) => {
                for id in ids {
                    self.health.remove(id);
                    self.max_health.remove(id);
                    self.bosses.remove(id);
                    self.names.remove(id);
                    self.kinds.remove(id);
                    self.horse_flags.remove(id);
                }
            }
            _ => {}
        }
    }

    /// `EntityPlayerSP.isRidingHorse`: montado num cavalo selado.
    /// A entidade é um cavalo (`EntityHorse`).
    pub fn is_horse(&self, id: i32) -> bool {
        self.kinds.get(&id) == Some(&HORSE)
    }

    pub fn is_horse_saddled(&self, id: i32) -> bool {
        self.kinds.get(&id) == Some(&HORSE) && self.horse_flags.get(&id).is_some_and(|f| f & 4 != 0)
    }

    /// Vida atual e máxima de uma montaria (`EntityLivingBase`).
    pub fn mount(&self, id: i32) -> Option<crate::hud_stats::MountHealth> {
        let health = *self.health.get(&id)?;
        Some(crate::hud_stats::MountHealth { health, max_health: self.max_health.get(&id).copied().unwrap_or(20.0) })
    }
}

/// 0x20 Entity Properties: devolve (entidade, valor final de `generic.maxHealth`) se o pacote o traz.
pub fn parse_max_health(body: &[u8]) -> mc_protocol::Result<Option<(i32, f32)>> {
    let mut r = Reader::new(body);
    let entity = r.varint()?;
    let n = r.i32()?;
    let mut out = None;
    for _ in 0..n.clamp(0, 1024) {
        let key = r.string()?;
        let base = r.f64()?;
        let mut mods = Vec::new();
        for _ in 0..r.varint()?.clamp(0, 1024) {
            r.bytes(16)?; // UUID do modificador
            mods.push((r.f64()?, r.u8()?));
        }
        if key == "generic.maxHealth" {
            out = Some((entity, compute_attribute(base, &mods, 0.0, 1024.0) as f32));
        }
    }
    Ok(out)
}

/// 0x20 do próprio jogador: (entidade, base, modificadores) de `generic.movementSpeed`, se o pacote o traz.
#[allow(clippy::type_complexity)]
pub fn parse_speed(body: &[u8]) -> mc_protocol::Result<Option<(i32, f64, Vec<(u128, f64, u8)>)>> {
    let mut r = Reader::new(body);
    let entity = r.varint()?;
    let n = r.i32()?;
    let mut out = None;
    for _ in 0..n.clamp(0, 1024) {
        let key = r.string()?;
        let base = r.f64()?;
        let mut mods = Vec::new();
        for _ in 0..r.varint()?.clamp(0, 1024) {
            let id = u128::from_be_bytes(r.bytes(16)?.try_into().unwrap_or([0; 16]));
            mods.push((id, r.f64()?, r.u8()?));
        }
        if key == "generic.movementSpeed" {
            out = Some((entity, base, mods));
        }
    }
    Ok(out)
}

/// `ModifiableAttributeInstance.computeValue`: operação 0 soma na base, 1 soma `base * x`, 2 multiplica por `1 + x`.
pub fn compute_attribute(base: f64, mods: &[(f64, u8)], min: f64, max: f64) -> f64 {
    let mut d0 = base;
    for (a, op) in mods {
        if *op == 0 {
            d0 += a;
        }
    }
    let mut d1 = d0;
    for (a, op) in mods {
        if *op == 1 {
            d1 += d0 * a;
        }
    }
    for (a, op) in mods {
        if *op == 2 {
            d1 *= 1.0 + a;
        }
    }
    d1.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s20(entity: u8, props: &[(&str, f64, &[(f64, u8)])]) -> Vec<u8> {
        let mut b = vec![entity];
        b.extend((props.len() as i32).to_be_bytes());
        for (k, v, mods) in props {
            b.push(k.len() as u8);
            b.extend(k.as_bytes());
            b.extend(v.to_be_bytes());
            b.push(mods.len() as u8);
            for (a, op) in *mods {
                b.extend([0u8; 16]);
                b.extend(a.to_be_bytes());
                b.push(*op);
            }
        }
        b
    }

    #[test]
    fn parse_0x20_vida_maxima() {
        let b = s20(7, &[("generic.movementSpeed", 0.1, &[]), ("generic.maxHealth", 20.0, &[(4.0, 0)])]);
        assert_eq!(parse_max_health(&b).unwrap(), Some((7, 24.0)));
        // Impulso de vida (op 1: +50% da base).
        let b = s20(7, &[("generic.maxHealth", 20.0, &[(0.5, 1)])]);
        assert_eq!(parse_max_health(&b).unwrap(), Some((7, 30.0)));
        // Sem vida máxima no pacote.
        assert_eq!(parse_max_health(&s20(7, &[("generic.movementSpeed", 0.1, &[])])).unwrap(), None);
        assert!(parse_max_health(&[7, 0, 0]).is_err());
    }

    #[test]
    fn metadados_do_jogador_e_chefe() {
        let mut t = Track::new();
        t.packet(&EntityPacket::Metadata { id: 5, meta: vec![(17, MetaValue::Float(4.0)), (1, MetaValue::Short(120)), (6, MetaValue::Float(18.5))] }, Some(5));
        assert_eq!((t.absorption, t.air, t.health[&5]), (4.0, 120, 18.5));
        // Metadado 17 de outra entidade não vira absorção do jogador.
        t.packet(&EntityPacket::Metadata { id: 6, meta: vec![(17, MetaValue::Float(9.0))] }, Some(5));
        assert_eq!(t.absorption, 4.0);
        t.packet(
            &EntityPacket::SpawnMob { id: 9, kind: WITHER, x: 0, y: 0, z: 0, yaw: 0, pitch: 0, head_yaw: 0, vel: (0, 0, 0), meta: vec![(6, MetaValue::Float(150.0)), (2, MetaValue::Str("Boss".into()))] },
            Some(5),
        );
        assert_eq!(t.bosses[&9], WITHER);
        assert_eq!(t.max_health[&9], 300.0);
        assert_eq!(t.names[&9], "Boss");
        let m = t.mount(9).unwrap();
        assert_eq!((m.health, m.max_health), (150.0, 300.0));
        t.packet(&EntityPacket::Destroy(vec![9]), Some(5));
        assert!(t.mount(9).is_none() && t.bosses.is_empty());
    }
}
