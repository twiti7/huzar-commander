# Huzar Commander

> **Język / Language:** [Polski](README_PL.md) | [English](README.md)

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#licencja)
[![Rust: 2021](https://img.shields.io/badge/Rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![GUI: eframe/egui 0.31](https://img.shields.io/badge/GUI-eframe%2Fegui%200.31-green.svg)](https://github.com/emilk/egui)
[![Tests: 85 passing](https://img.shields.io/badge/Tests-85%20passing-brightgreen.svg)](#testy-jednostkowe)

Nowoczesny, dwupanelowy menedżer plików GUI w języku Rust wzorowany na Norton Commander, Total Commander i FAR Manager. Nazwa nawiązuje do polskiej husarii – dwa rozpostarte skrzydła symbolizują dwa panele, pancerz to gwarancja bezpieczeństwa pamięci Rusta, a szybkość szarży to błyskawiczne operacje I/O z telemetrią w czasie rzeczywistym.

![Huzar Commander Screenshot](assets/screenshot.png)

## Funkcjonalności

- **Układ dwupanelowy (Dual-pane):** Lewy i prawy panel z niezależną nawigacją, podświetleniem aktywnego okna i precyzyjnym obcinaniem krawędzi (panele nigdy na siebie nie nachodzą).
- **Asynchroniczne wczytywanie katalogów z możliwością przerwania (Esc):** Płynne, nieblokujące skanowanie folderów i dysków sieciowych (SMB/UNC np. `\\serwer\udzial`) w tle. Jeśli łączenie lub odczyt zasobu trwa długo, interfejs nigdy nie zamarza – wyświetlany jest estetyczny ekran ładowania z animowanym spinnerem, stoperem oraz przyciskiem `[ ✖ Anuluj (Esc) ]`, który natychmiast przerywa oczekiwanie i przywraca poprzedni katalog, a drugi panel pozostaje cały czas w pełni responsywny.
- **Wybór dysków (Alt+F1 / Alt+F2):** 
  - Klawisz `Alt+F1` otwiera okno wyboru dysku dla lewego panelu.
  - Klawisz `Alt+F2` otwiera okno wyboru dysku dla prawego panelu.
  - Bezpośredni wybór po naciśnięciu litery dysku (np. klawisz `C` lub `D` natychmiast zmienia dysk).
  - Nawigacja strzałkami `Góra`/`Dół` + `Enter` lub kliknięcie myszą.
  - Wyświetlanie etykiety woluminu (np. `Windows`, `Dane`), wolnego i całkowitego miejsca oraz wskaźnika zajętości.
  - Przyciski `🖴 Alt+F1` oraz `🖴 Alt+F2` bezpośrednio na pasku dysków każdego panelu.
- **Pasek dysków:** Szybkie przełączanie dysków (`[-C-]`, `[-D-]`, itp.) z podświetleniem aktywnego dysku.
- **Tabela plików (Virtual Scrolling):** Płynne przewijanie z automatycznym śledzeniem pozycji kursora (`scroll_to_row`). Kolumny: Nazwa (elastyczna), Rozszerzenie, Rozmiar (do prawej), Data modyfikacji (do prawej).
- **Wielobarwne kodowanie plików:**
  - 📁 Katalogi i `[..]` na złoto/żółto
  - 🟢 Pliki `.exe`, `.bat`, `.cmd` na jasnozielono
  - 🟠 Archiwa `.zip`, `.rar`, `.7z` na pomarańczowo
  - 🔵 Pliki `.rs`, `.toml`, `.txt` na błękitno
  - 🔴 Zaznaczone pliki na czerwono (klasyka Total Commandera)
- **Sortowanie:** Kliknięcie nagłówka kolumny (Nazwa, Rozszerzenie, Rozmiar, Data) sortuje listę, zachowując pozycję kursora na tym samym pliku.
- **Zaznaczanie:** 
  - Klawisz `Insert` zaznacza plik i przesuwa kursor w dół.
  - Klawisz `Spacja` zaznacza/odznacza element i przesuwa kursor w dół; dla folderów uruchamia asynchroniczne zliczanie rozmiaru w tle (ze wskaźnikiem `<...>` w kolumnie Rozmiar) bez blokowania aplikacji.
  - Prawy przycisk myszy (RMB) zaznacza/odznacza plik pod kursorem.
  - Kliknięcie w dowolne miejsce wiersza (pełny wiersz) zmienia kursor i aktywuje panel.
  - Menu *Zaznacz*: zaznacz wszystko, odznacz wszystko, odwróć zaznaczenie.
  - Pasek stanu na górze każdego panelu podsumowuje liczbę i rozmiar zaznaczonych elementów.
- **Kopiowanie z telemetrią (F5):**
  - **Prędkość dyskowa SSD:** Kopiowanie blokami 1 MB bez zbędnych narzutów.
  - **Wykres prędkości w czasie:** Neonowy wykres liniowy w czasie rzeczywistym z dynamicznym skalowaniem szczytu.
  - **Podwójny pasek postępu:** Postęp bieżącego pliku oraz postęp całkowity zadania.
  - **Szacowany czas do końca (ETA):** Dokładny licznik `GG:MM:SS` wyliczany z prędkości EMA i pozostałych bajtów.
  - **Czas trwania:** Dokładny stoper operacji.
  - **Praca w tle (`W tle`):** Możliwość zminimalizowania okna transferu do małego paska na dole i swobodnego korzystania z całego programu podczas kopiowania!
  - **Pokaż szczegóły:** Błyskawiczny powrót z paska do pełnego okna z wykresem.
  - **Anuluj:** Natychmiastowe przerwanie transferu w dowolnym momencie.
- **Kolejka operacji w tle (Background Transfer Manager):**
  - Wielozadaniowość: nowe operacje kopiowania/przenoszenia zlecane w trakcie trwania transferu są natychmiast dynamicznie kolejkowane bez czekania na zakończenie poprzednich zadań.
  - Agregowany czas i rozmiar: dynamiczne przeliczanie całkowitego rozmiaru kolejki i łącznego wskaźnika ETA w czasie rzeczywistym.
  - Sterowanie kolejką: przyciski wstrzymania (`Pauza`), wznowienia (`Wznów`), natychmiastowego pomijania bieżącego zadania (`⏭ Pomiń`) dostępne w oknie transferu, w dolnym pasku oraz w oknie kolejki, a także anulowania całej kolejki.
  - Dedykowane okno kolejki z listą zadań oczekujących i bieżącą telemetrią.
- **Wyszukiwanie plików (Alt+F7):**
  - Wyszukiwanie wielowątkowe bez blokowania interfejsu użytkownika.
  - Maski plików i wzorce wieloznaczne (np. `*.rs`, `*test*`, `*.txt;*.md`).
  - Przeszukiwanie zawartości tekstowej wewnątrz plików (z opcją ignorowania lub rozróżniania wielkości liter).
  - Wyniki w czytelnej tabeli z podglądem trafienia, ścieżki i rozmiaru.
  - Przyciski "Przejdź do pliku" (natychmiastowe przejście do folderu w panelu i ustawienie kursora) oraz "Podgląd (F3)".
- **Obsługa archiwów ZIP:**
  - Przeglądanie archiwów ZIP tak jak zwykłych katalogów (wejście klawiszem `Enter` lub dwuklikiem).
  - Wirtualna nawigacja po podkatalogach wewnątrz archiwum i wyjście do folderu nadrzędnego (`[..]` / `Backspace`).
  - Szybki podgląd plików wewnątrz archiwum pod klawiszem `F3`.
  - Rozpakowywanie wybranych plików lub całego archiwum (`F5`) do przeciwnego panelu.
- **Szybkie filtrowanie widoku plików na żywo:**
  - Wbudowane pole filtra `🔍` bezpośrednio w nagłówku każdego panelu (skrót `Ctrl+F`).
  - Filtrowanie w czasie rzeczywistym wg masek (`*.rs`), wielu rozszerzeń (`*.rs;*.toml`) lub podciągów znaków.
  - Czyszczenie filtra jednym kliknięciem `✖` lub klawiszem `Esc`.
- **Przenoszenie / Zmiana nazwy (F6):** Błyskawiczne atomowe przenoszenie na tym samym dysku lub kopiowanie z usuwaniem między różnymi woluminami.
- **Tworzenie folderu (F7):** Okno dialogowe z polem nazwy nowego folderu.
- **Usuwanie (F8 / Delete):** Z opcją bezpiecznego przeniesienia do systemowego Kosza.
- **Szybki podgląd (F3):** Wbudowany podglądacz tekstowy w oknie modalnym (obsługa widoku Text, Hex, Markdown, JSON, YAML).
- **Edycja (F4):** Otwarcie w systemowym Notatniku (lub skonfigurowanym edytorze).
- **Wiersz poleceń na dole:** Uruchamianie dowolnych komend systemu operacyjnego w bieżącej ścieżce aktywnego panelu.

## Klawisze skrótów

| Klawisz | Akcja |
|---|---|
| `Tab` | Przełączenie aktywnego panelu (Lewy <-> Prawy) |
| `Enter` | Wejście do katalogu / Uruchomienie skojarzonego pliku |
| `Backspace` | Wyjście do katalogu nadrzędnego (`..`) |
| `Strzałki Góra/Dół` | Poruszanie kursorem z auto-przewijaniem widoku |
| `PageUp / PageDown` | Przeskok o 15 wierszy w górę / w dół |
| `Home / End` | Przeskok na początek / koniec listy |
| `Insert` | Zaznaczenie pliku i przejście o 1 wiersz niżej |
| `Spacja` | Zaznaczenie elementu / Obliczenie rozmiaru folderu w tle i przejście w dół |
| `Prawy Przycisk Myszy` | Zaznaczenie / odznaczenie pliku pod kursorem |
| `Kliknięcie w wiersz` | Ustawienie kursora na pliku i aktywacja panelu |
| `Dwuklik Lewy Przycisk` | Wejście do katalogu / Otwarcie pliku |
| `F1` | Pomoc / O programie (informacje o wersji, technologiach i skrótach) |
| `F3` | Szybki podgląd pliku |
| `F4` | Edycja pliku w Notatniku |
| `F5` | Kopiowanie (z wykresem, ETA i opcją pracy w tle) |
| `F6` | Przenoszenie / Zmiana nazwy |
| `F7` | Tworzenie nowego folderu |
| `F8` lub `Delete` | Usuwanie plików / folderów |
| `Alt+F1` | Wybór dysku lewego panelu (okno dysków) |
| `Alt+F2` | Wybór dysku prawego panelu (okno dysków) |
| `Alt+F7` | Wyszukiwanie plików (po nazwie i zawartości) |
| `Ctrl+F` | Szybkie filtrowanie plików w aktywnym panelu |
| `Esc` | Wyczyszczenie filtra / Zamknięcie okna dialogowego |
| `Alt+F4` | Wyjście z programu |

## Uruchamianie i Kompilacja

### Windows
W terminalu PowerShell w folderze projektu:

* **Wersja deweloperska (Debug):**
  ```powershell
  .\run.ps1
  # lub:
  cargo run
  ```

* **Budowanie wersji produkcyjnej (Release):**
  ```powershell
  .\build_release.ps1
  # lub bezpośrednio przez cargo:
  cargo build --release
  ```
  Skompilowany plik `.exe` znajdziesz w:
  `target\release\huzar-commander.exe`

* **Uruchomienie wersji Release:**
  ```powershell
  .\run_release.ps1
  # lub:
  cargo run --release
  ```

* **Budowanie uniwersalnego instalatora Windows (.exe):**
  ```powershell
  .\build_installer.ps1
  ```
  Generuje gotowy plik instalatora `target\installer\HuzarCommander-Setup-v0.1.0.exe` (Inno Setup) z wyborem języka (PL/EN), skrótami na Pulpicie i w Menu Start, deinstalatorem w Ustawieniach Windows oraz integracją z menu kontekstowym Eksploratora Windows (*„Otwórz w programie Huzar Commander”*).

### Linux / Ubuntu / Debian
Przed pierwszym uruchomieniem należy upewnić się, że zainstalowane są standardowe biblioteki okienkowe (X11 / Wayland / OpenGL):
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libxkbcommon-dev libgl1-mesa-dev libfontconfig1-dev
```
Uruchomienie:
```bash
chmod +x ./run.sh
./run.sh
# lub bezpośrednio:
cargo run
```

Na Linuksie menedżer automatycznie wykrywa:
- Główny system plików `/` (skrót `[-R-]` / klawisz `R` lub `/`),
- Folder użytkownika `$HOME` (skrót `[-H-]` / klawisz `H`),
- Wszystkie fizyczne i sieciowe punkty montowania z `/proc/mounts`, `/mnt` oraz `/media` (ze skrótami literowymi `[-A-]`, `[-B-]`, itp.).
- Pojemności i wolne miejsce odczytywane są standardowym wywołaniem POSIX `statvfs`.

## Testy jednostkowe i integracyjne

Projekt posiada rozbudowany zestaw **86 zautomatyzowanych testów** (15 modułów testowych w katalogu `tests/`) pokrywających:
- Parsowanie i atrybuty plików (`fs_item`),
- Sortowanie dwupanelowe, zaznaczanie oraz asynchroniczne wczytywanie katalogów z przerwaniem (`panel`),
- Operacje zaznaczania (`toggle_select`, `select_all`, `deselect_all`, `invert_selection`) oraz przeliczanie sumarycznych rozmiarów (`panel`),
- Wykrywanie dysków systemowych i odczyt pojemności (`drives`),
- Asynchroniczny silnik I/O: tworzenie katalogów, kopiowanie i przenoszenie plików, obsługa kolejki zadań (`worker`),
- Archiwa ZIP: podgląd struktury wirtualnej, kompresja i dekompresja (`zip_support`),
- Moduł internacjonalizacji i18n: parzystość kluczy PL/EN, detekcja języka i formatowanie (`i18n`),
- Silniki podglądu: Markdown, JSON/YAML, diagramy Mermaid, Hex dumper oraz strumieniowy bufor 64 KB (`viewer`).

Uruchomienie testów:
```powershell
cargo test
```

## Licencja

Projekt jest udostępniany na zasadach podwójnej licencji (Dual License), standardowej w ekosystemie Rust:

* **MIT License** ([LICENSE-MIT](LICENSE-MIT))
* **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

Użytkownik może wybrać dowolną z tych licencji według własnych preferencji.
