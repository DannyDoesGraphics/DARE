const uint Normal = 0x1;
const uint Tangent = 0x2;
const uint UV = 0x3;

/// Checks if a given bit flag is set
bool isFlagSet(in uint bit, in uint flag) {
    return (bit & flag) != 0;
}