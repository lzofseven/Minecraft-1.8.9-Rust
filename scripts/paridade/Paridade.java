// Referência numérica do movimento do jogador 1.8.9 em chão plano infinito.
// As contas são as do código decompilado (EntityPlayerSP/EntityPlayer/EntityLivingBase/
// Entity.moveEntity), em Java, com float/double e o MathHelper de tabela do jogo.
// O chão é o topo do bloco y=63 (pés em y=64). Gera mc-client/src/physics/paridade.rs.
// Uso: java scripts/paridade/Paridade.java > mc-client/src/physics/paridade.rs
public class Paridade {
    static final float[] SIN_TABLE = new float[65536];
    static { for (int i = 0; i < 65536; ++i) SIN_TABLE[i] = (float)Math.sin((double)i * Math.PI * 2.0D / 65536.0D); }
    static float sin(float f) { return SIN_TABLE[(int)(f * 10430.378F) & 65535]; }
    static float cos(float f) { return SIN_TABLE[(int)(f * 10430.378F + 16384.0F) & 65535]; }

    double posX, posY = 64.0, posZ, motionX, motionY = (0.0D - 0.08D) * 0.9800000190734863D, motionZ;
    float yaw, jumpMovementFactor = 0.02F, flySpeed = 0.05F;
    boolean onGround = true, sprinting, flying, sneak, jump;
    float moveForward, moveStrafe;
    int jumpTicks;

    double speed() { double d = 0.10000000149011612D; if (sprinting) d *= 1.0D + 0.30000001192092896D; return d; }

    void moveFlying(float strafe, float forward, float friction) {
        float f = strafe * strafe + forward * forward;
        if (f >= 1.0E-4F) {
            f = (float)Math.sqrt((double)f);
            if (f < 1.0F) f = 1.0F;
            f = friction / f; strafe = strafe * f; forward = forward * f;
            float f1 = sin(yaw * 3.1415927F / 180.0F), f2 = cos(yaw * 3.1415927F / 180.0F);
            motionX += (double)(strafe * f2 - forward * f1);
            motionZ += (double)(forward * f2 + strafe * f1);
        }
    }

    // Caixa do jogador: a posição sai dela (setPosition / resetPositionToBB), como no Entity.
    double minX, minY, minZ, maxX, maxY, maxZ;
    void setPos(double x, double y, double z) {
        posX = x; posY = y; posZ = z;
        float f = 0.6F / 2.0F, f1 = 1.8F;
        minX = x - (double)f; minY = y; minZ = z - (double)f; maxX = x + (double)f; maxY = y + (double)f1; maxZ = z + (double)f;
    }

    void moveEntity(double x, double y, double z) {
        double d4 = y;
        if (minY + y < 64.0D && minY >= 64.0D) y = 64.0D - minY; // chão (calculateYOffset)
        minY += y; maxY += y;
        minX += x; maxX += x;
        minZ += z; maxZ += z;
        posX = (minX + maxX) / 2.0D; posY = minY; posZ = (minZ + maxZ) / 2.0D;
        boolean collidedV = d4 != y;
        onGround = collidedV && d4 < 0.0D;
        if (collidedV) motionY = 0.0D;
    }

    void moveEntityWithHeading(float strafe, float forward) {
        float f4 = 0.91F;
        if (onGround) f4 = 0.6F * 0.91F;
        float f = 0.16277136F / (f4 * f4 * f4);
        float f5 = onGround ? (float)speed() * f : jumpMovementFactor;
        moveFlying(strafe, forward, f5);
        f4 = 0.91F;
        if (onGround) f4 = 0.6F * 0.91F;
        moveEntity(motionX, motionY, motionZ);
        motionY -= 0.08D;
        motionY *= 0.9800000190734863D;
        motionX *= (double)f4;
        motionZ *= (double)f4;
    }

    void tick(float forward, float strafe) {
        moveForward = forward; moveStrafe = strafe;
        if (sneak) { moveStrafe = (float)((double)moveStrafe * 0.3D); moveForward = (float)((double)moveForward * 0.3D); }
        if (flying) { if (sneak) motionY -= (double)(flySpeed * 3.0F); if (jump) motionY += (double)(flySpeed * 3.0F); }
        if (jumpTicks > 0) --jumpTicks;
        if (Math.abs(motionX) < 0.005D) motionX = 0.0D;
        if (Math.abs(motionY) < 0.005D) motionY = 0.0D;
        if (Math.abs(motionZ) < 0.005D) motionZ = 0.0D;
        if (jump && !flying) {
            if (onGround && jumpTicks == 0) {
                motionY = (double)0.42F;
                if (sprinting) { float f = yaw * 0.017453292F; motionX -= (double)(sin(f) * 0.2F); motionZ += (double)(cos(f) * 0.2F); }
                jumpTicks = 10;
            }
        } else if (!jump) jumpTicks = 0;
        float s = moveStrafe * 0.98F, fw = moveForward * 0.98F;
        if (flying) {
            double d3 = motionY; float f = jumpMovementFactor;
            jumpMovementFactor = flySpeed * (float)(sprinting ? 2 : 1);
            moveEntityWithHeading(s, fw);
            motionY = d3 * 0.6D; jumpMovementFactor = f;
        } else moveEntityWithHeading(s, fw);
        jumpMovementFactor = 0.02F;
        if (sprinting) jumpMovementFactor = (float)((double)jumpMovementFactor + (double)0.02F * 0.3D);
        if (onGround && flying) flying = false;
    }

    static void series(String name, Paridade p, int ticks, float forward) {
        p.setPos(p.posX, p.posY, p.posZ);
        System.out.print("pub const " + name + ": [[f64; 3]; " + ticks + "] = [");
        for (int i = 0; i < ticks; ++i) { p.tick(forward, 0.0F); System.out.print("[" + p.posX + ", " + p.posY + ", " + p.posZ + "],"); }
        System.out.println("];");
    }

    public static void main(String[] a) {
        System.out.println("// GERADO por scripts/paridade/Paridade.java (contas do Java 1.8.9). Não editar.");
        System.out.println("// Posição dos pés por tick, saindo de (0, 64, 0) com yaw 0 no chão plano (topo em y=64).");
        Paridade p = new Paridade(); series("ANDAR", p, 40, 1.0F);
        p = new Paridade(); p.sprinting = true; series("CORRER", p, 40, 1.0F);
        p = new Paridade(); p.sneak = true; series("AGACHAR", p, 40, 1.0F);
        p = new Paridade(); p.jump = true; series("PULAR", p, 12, 0.0F);
        p = new Paridade(); p.jump = true; p.sprinting = true; series("CORRER_PULANDO", p, 40, 1.0F);
        p = new Paridade(); p.onGround = false; p.motionY = 0.0D; p.posY = 64.0 + 20.0; series("CAIR", p, 40, 0.0F);
        p = new Paridade(); p.onGround = false; p.motionY = 0.0D; p.posY = 70.0; p.flying = true; series("VOAR", p, 40, 1.0F);
        p = new Paridade(); p.onGround = false; p.motionY = 0.0D; p.posY = 70.0; p.flying = true; p.sprinting = true; series("VOAR_CORRENDO", p, 40, 1.0F);
        p = new Paridade(); p.onGround = false; p.motionY = 0.0D; p.posY = 66.0; p.flying = true; p.sneak = true; series("VOO_DESCER", p, 20, 0.0F);
        p = new Paridade(); p.onGround = false; p.motionY = 0.0D; p.posY = 70.0; p.flying = true; p.jump = true; series("VOO_SUBIR", p, 20, 0.0F);
    }
}
