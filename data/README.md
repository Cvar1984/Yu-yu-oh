# data

| File | Contents | In git |
| --- | --- | --- |
| `cards.json` | The 535 cards of the owner's game: LOB, MRD, SRL and PSV (Pharaoh's Servant, added 2026-11-21) (479) plus 56 EXTRA cards; `in_game_index` says which list each sits under | yes, from the data bundle |
| `extra_cards.txt` | Names of the 56 EXTRA cards, for `refresh_cards.py` | yes |
| `banlist.json` | Copy limits (0, 1 or 2) from the owner's game, kept by hand; every player follows them | yes |
| `tokens.json` | Tokens cards create (Scapegoat's Sheep Token): id and name from YGOPRODeck, stats from the making card's text; never in a deck or collection (owner, 2026-11-23) | yes |
| `collection.json` | Save A: the owner's cards and copy counts | yes, from the data bundle |
| `collection_save_b.json` | Save B | yes, from the data bundle |
| `decks/*.json` | Six enemy decks, save A's best overall (`save_a_best_overall_40`), control and current decks, and save B's best, with results in `notes`; `for` says whose cards a deck uses | yes |
| `../images/full`, `../images/small` | One JPEG per card id | no: Konami-owned, fetched locally |

Card id is the key everywhere: it is the card's official password, and every
other file refers to it. Names in `banlist.json` and `collection.json` are
comments for humans; the loader checks ids only.

`../tools/refresh_cards.py` rebuilds `cards.json` and downloads missing images.
On 2026-11-21 YGOPRODeck returned new ids for two EXTRA cards (La Jinn the
Mystical Genie of the Lamp 97590747, Ultimate Offering 80604091; the file
keeps 97590748 and 80604092, which decks and collections use), so the PSV
cards were merged in by hand from a refresh instead of replacing the file.
`../reference/python_prototype/` holds the Python simulator the handoff's
findings came from; its rules were simplified, so treat its numbers as
hypotheses.
