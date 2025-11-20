# Co jest

## Główna pętla - równanie kroku
### $x_{new} = x_{old}+ \frac{step}{T_{okrążenia}} \cdot L$

gdzie:

$x_{new}$ - nowa pozycja (metry)

$x_{old}$ - stara pozycja na torze

$step$ - krok czasowy (1 sekunda)

$T_{okrążenia}$ - Obliczany aktualny (teoretyczny) czas okrążenia bolidu (sekundy) (na podstawie zużycia opon, kierowcy)

$L$ -  długość toru

## Obliczanie teoretycznego czasu okrążenia
Na razie obliczany na podstawie straty podstawowej
### $$

## Obliczanie straty podstawowej
### $S_{pods} = S_{bolid} + S_{kierowca} + S_{opony}$

$S_{bolid}$ -  parametr straty związany ze specyfikacją samochodu (na razie wpisywany na sztywno)

$S_{keirowca}$ - parametr straty związany z umiejętnościami kierowcy. W dalszych etapach będzie on wywnioskowany na podstawie datasetu kierowców F1 i ich wyników w ostatnich latach.

### $S_{opony} = k_0 + k_1 * age$
$k_0$ - początkowy "offset" opon. Świeże opony mogą być dużo szybsze na początku (wtedy jest ujemny)

$k_1$ - współczynnik degradacji (strata sekund na okrążenie)

$age$ - wiek opon (całkowity, w okrążeniach)


## Wizualizacja w GUI
Do wizualizacji pozycji bolidu na torze (który jest zdefiniowany jako zbiór punktów CSV), symulator używa interpolacji liniowej do znalezienia dokładnych współrzędnych (x, y) na podstawie przebytego dystansu (s). Pozwala to zwizualizować gdzie będzie pojazd na mapie jak przejechał na przykład 50.5 metra. Domyślnie mapa toru jest dyskretna w metrach.

## Pit Stop
- Na początku wyścigu definiujemy **strategię** pit stop dla każdego kierowcy, na przykład:
	- Kierowca zaczyna z twardymi oponami
	- po 15 okrążeniach zjeżdża do zmiany opon na medium
	- po 20 okrążeniach zmienia spowrotem na twarde
- Symulacja sprawdza po każdym okrążeniu czy dany bolid planuje Pit Stop
	- jeśli tak to pojazd zjeżdża do pit stop i jego stan zmienia się z "OnTrack" na "Pitlane"
- Jeśli pojazd jest w "Pitlane" normalna pętla aktualizująca jego przemieszczenie przestaje działać i pojazd zwalnia
- Jak zjedzie do pit stopu to jest hard codowany czas postoju 2.5s (w poźniejszej wersji będzie to obliczane)
- Ruszanie jest zaimplementowane odwrotnie co hamowanie
# Co dodamy (niektóre rzeczy już są w trakcie wdrażania)
- Wpływ paliwa na wyścig - postoje na tankowanie oraz waga jak waga paliwa wpływa na masę i zarazem prędkość pojazdu
- Personalizowane statystyki samochodów (teamów) jak i kierowców wzięte z realnych baz danych
- Bardziej rzeczywisty mechanizm wyprzedzania (specjalne strefy gdzie można wyprzedzać)
- Wprowadzenie specjalnych stref i w nich pojazdy będą rozwijały różną prędkość (na zakrętach nie będą tak szykie jak na prostej (overtaking lane))
- Możliwe (rzadkie ale możliwe) wypadki na torze (prawdopodobieństwo zależne od kierowcy). Będą one możliwe gdy samochód będzie w stanie "walki" z innym bolidem.
