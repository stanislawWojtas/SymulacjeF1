## Co dodane
### Awarie silnika - szansa na to 0.1%
- jeżeli jakieś auto jest DNF to wkracza sefety car (na razie uproszczone bo na do końca wyścigu)
### Safety car - stan SC:
- podczas awarii safety car wjeżdża na wyścig (przed lidera)
- wszyscy zwalniają
- nie można się wyprzedzać
- samochody wyrównują pozycje

### Zmiana modelu opon - teraz nie zużywają się liniowo
- na początku zużycie liniowe dopóki nie trafią na klif
- jeżeli jest granica (klif) to opony zużywają się coraz szybciej
- jest minimalna wartość degradacji żeby samochód nie zwolnił do zera

### Interakcja między samochodami
- jeżeli pojazd jedzie za innym to zwiększane jest zużycie opon (brudne powierze)
- na zakrętach nie można wyprzedzać
- jazda w tłoku niszczy opony 2x szybciej

### Pogoda


# Dobra jaki problem rozwiązujemy
- No na pewno jest model stochastyczny - są zdarzenia losowe (awarie silnika modelowane rozkłądem Poissona, losowe zmiany pogody, błędy kierowców)
- Symulacja monte carlo - wyścig można uruchomić z takimi samymi parametrami kilka razy i uśrednić wyniki
- 