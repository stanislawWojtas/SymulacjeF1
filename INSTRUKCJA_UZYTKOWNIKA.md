# Symulator Wyścigu F1 - Instrukcja Użytkownika

## Opis
Symulator wyścigu Formuły 1 z dyskretnymi krokami czasowymi dla **2 kierowców** (DRV1 vs DRV2).

## Wymagania
- Rust (cargo)
- System Windows/Linux/macOS

## Instalacja i Kompilacja
```bash
cd time-discrete-race-simulator
cargo build --release
```

## Uruchamianie

### Tryb GUI (z wizualizacją)
```bash
cargo run -- -g
```
Lub z release:
```bash
cargo run --release -- -g
```

**Uwaga**: GUI wymaga pliku toru `input/tracks/YasMarina.csv`

### Tryb bez GUI (tylko wyniki w konsoli)
```bash
cargo run
```

### Dostępne opcje

| Opcja | Skrót | Opis | Domyślna wartość |
|-------|-------|------|------------------|
| `--gui` | `-g` | Uruchamia wizualizację GUI | wyłączona |
| `--debug` | `-d` | Włącza szczegółowy debug | wyłączony |
| `--timestep-size` | `-t` | Krok czasowy symulacji (s) | 0.1 |
| `--realtime-factor` | `-r` | Mnożnik czasu rzeczywistego (GUI) | 1.0 |

### Przykłady

**Szybsza symulacja GUI (2x prędkość):**
```bash
cargo run -- -g -r 2.0
```

**Wolniejsza symulacja z mniejszym krokiem:**
```bash
cargo run -- -g -t 0.05 -r 0.5
```

**Tryb debug bez GUI:**
```bash
cargo run -- -d
```

## Parametry Symulacji (hardcoded)

### Tor: YasMarina
- Długość: 5554 m
- Liczba okrążeń: 30
- Bazowy czas okrążenia: ~95s

### Kierowcy
- **DRV1** (Driver One) - Bolid #1 (czerwony)
  - Strata: +0.1s/okrążenie
  - Strategia: Start na MEDIUM → Pit stop okr. 15 → HARD
  - Pozycja startowa: P1

- **DRV2** (Driver Two) - Bolid #2 (niebieski)  
  - Strata: 0.0s/okrążenie (bazowy)
  - Strategia: Start na SOFT → Pit stop okr. 12 → MEDIUM
  - Pozycja startowa: P2

### Mieszanki opon
- **SOFT**: Szybka na początku, szybka degradacja (0.05s/okr)
- **MEDIUM**: Zrównoważona, średnia degradacja (0.03s/okr)
- **HARD**: Wolna na początku, niska degradacja (0.02s/okr)

## Modyfikacja Parametrów

Aby zmienić parametry (liczba okrążeń, strategia, tor itp.), edytuj funkcję `get_hardcoded_sim_pars()` w pliku:
```
racesim/src/pre/read_sim_pars.rs
```

## Struktura Projektu

```
time-discrete-race-simulator/
├── cli/           # Interfejs wiersza poleceń
├── gui/           # Moduł wizualizacji
├── racesim/       # Główny moduł symulacji
│   └── src/
│       ├── core/  # Logika symulacji
│       ├── pre/   # Parametry i konfiguracja
│       └── post/  # Przetwarzanie wyników
├── helpers/       # Funkcje pomocnicze
└── input/
    └── tracks/    # Pliki CSV z torami
```

## Wyniki

### Tryb GUI
- Wizualizacja 2D toru wyścigowego
- Pozycje samochodów w czasie rzeczywistym
- Numer okrążenia i czas wyścigu

### Tryb konsoli
- Tabela czasów okrążeń dla każdego kierowcy
- Tabele skumulowanych czasów wyścigu
- Czas wykonania symulacji

## Rozwiązywanie Problemów

**Problem**: Błąd "Could not open track file"
- **Rozwiązanie**: Upewnij się, że plik `input/tracks/YasMarina.csv` istnieje

**Problem**: GUI nie uruchamia się
- **Rozwiązanie**: Sprawdź czy masz zainstalowane zależności graficzne (OpenGL)

**Problem**: Kompilacja kończy się błędem
- **Rozwiązanie**: Uruchom `cargo clean` a następnie `cargo build`

## Autor
Alexander Heilmeier (TUM)  
Modyfikacje: Projekt studencki - Symulacja Systemów Dyskretnych
