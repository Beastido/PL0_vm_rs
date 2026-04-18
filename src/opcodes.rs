use std::fmt::{Display};
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    // Ein Argument sind die zwei Bytes, die auf den Opcode im Bytecode folgen.

    // Argumente: Adresse
    PushValueLocalVar = 0x0,
    // Argumente: Adresse
    PushValueMainVar = 0x1,
    // Argumente: Adresse, Prozeduren-ID
    PushValueGlobalVar = 0x2,
    // Argumente: Adresse
    PushAddressLocalVar = 0x3,
    // Argumente: Adresse
    PushAddressMainVar = 0x4,
    // Argumente: Adresse, Prozeduren-ID
    PushAddressGlobalVar = 0x5,
    // Argumente: Konstanten-ID
    PushConstant = 0x6,

    // auf Stack: oben = Wert, darunter = Zieladresse
    StoreValue = 0x7,
    // auf Stack: oben = Wert
    OutputValue = 0x8,
    // auf Stack: oben = Zieladresse
    InputToAddr = 0x9,

    // Operatoren mit 1 Faktor
    // auf Stack: oben = Wert → Ergebnis auf Stack: -Wert
    Minusify = 0xA,
    // auf Stack: oben = Wert → Ergebnis auf Stack: true (1) / false (0)
    IsOdd = 0xB,

    // Operatoren mit 2 Faktoren
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 + Wert 2
    OpAdd = 0xC,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 - Wert 2
    OpSubtract = 0xD,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 * Wert 2
    OpMultiply = 0xE,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 / Wert 2
    OpDivide = 0xF,

    // Boolean-Logik
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 == Wert 2
    CompareEq = 0x10,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 != Wert 2
    CompareNotEq = 0x11,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 < Wert 2
    CompareLT = 0x12,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 > Wert 2
    CompareGT = 0x13,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 ≤ Wert 2
    CompareLTEq = 0x14,
    // auf Stack: oben = Wert 2, darunter = Wert 1 → Ergebnis auf Stack: Wert 1 ≥ Wert 2
    CompareGTEq = 0x15,

    // Flow-Logik
    // Argumente: Prozedur-ID
    CallProc = 0x16,
    // keine Argumente
    ReturnProc = 0x17,
    // Argumente: Jump-Offset (auch negativ möglich)
    Jump = 0x18,
    // Argumente: Jump-Offset, auf Stack: oben = Wert, der false (0) sein kann
    JumpIfFalse = 0x19,
    // Argumente: Länge der Prozedur in Bytes, Prozedur-ID, Bytes auf Stack benötigt für Variablen
    EntryProc = 0x1A,

    //=== Erweiterungen ===
    // Argument: Null-terminierter String
    PutString = 0x1B,
    // auf Stack: oben = beliebiges Datum
    Pop = 0x1C,
    // auf Stack: oben = Adresse, die durch Daten an dieser ausgetauscht werden soll
    Swap = 0x1D,

    // nur für VM
    EndOfCode = 0x1E,

    // für dynamische Adressberechnung
    Put = 0x1F,
    Get = 0x20,
    OpAddAddr = 0x21,       // Funktionsweise unbekannt
    
    // Erweitertes Format mit Parameteranzahl als 4. Argument
    EntryProcEx = 0x22
}

impl Display for OpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!("{:?}", self))
    }
}
