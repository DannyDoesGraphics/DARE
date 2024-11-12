
/// Linear congruential generator
/// Also implies as the sequence continues, the number approaches 0
uint lcg(inout uint32_t prev) {
    uint LCG_A = 1664525u;
    uint LCG_C = 1013904223u;
    prev = (LCG_A * prev + LCG_C);
    return prev & uint(0x00FFFFFF);
}

/// Produces a random number [0, 1)
float rnd(inout uint32_t prev) {
    return (float(lcg(prev)) / float(0x01000000));
}