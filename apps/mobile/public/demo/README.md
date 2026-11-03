# Demo model attribution

`3DBenchy.stl` is the official single-part **#3DBenchy** model from
[3DBenchy.com](https://www.3dbenchy.com/), created by Daniel Norée and shared
by CreativeTools under the [CC0 1.0 Universal license](https://creativecommons.org/publicdomain/zero/1.0/).

Source: <https://github.com/CreativeTools/3DBenchy/blob/master/Single-part/3DBenchy.stl>

The checked-in source matches Git blob `5a51bb5f3fb335b4617f00b3ae5f3785690e13e5`.

`Flexi-Rex-improved.stl` is **Flexi Rex with stronger links** by DrLex, based
on Flexi Rex by Kirbs. It is distributed under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).

Source: <https://github.com/DrLex0/print3D-FlexiRex>

`NASA-Wrench.stl` is the ratchet wrench designed on Earth and transmitted to
the International Space Station for additive manufacturing. It is provided
by [NASA 3D Resources](https://science.nasa.gov/3d-resources/wrench-2/) under
NASA's public media usage guidelines.

Source: <https://github.com/nasa/NASA-3D-Resources/tree/master/3D%20Printing/Wrench>

`Voron-Stealthburner-main-body.stl` is the official Stealthburner main body
from Voron Design, distributed under the
[GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.en.html).

Source: <https://github.com/VoronDesign/Voron-Stealthburner>

Source file SHA-256 checksums:

```text
6ab57f1c3f8e86bc3cbd302c6fa6270acf06277c6335454e922419c25d42e97e  3DBenchy.stl
91c782360a4c22d9f12d51e3e6b7f3708ade0d79a86597f105b6ea863941d816  Flexi-Rex-improved.stl
2791ee75ee191ade5dfe8f7f498f0b6c150062da05b1c56f99944f97d2a32537  NASA-Wrench.stl
be070b34b71feffd2a3da4b8962854eed401fc946541b5764a7bf197d6f08994  Voron-Stealthburner-main-body.stl
```

The `.vlm5` viewer payloads are generated from those STL files with:

```sh
node scripts/generate-demo-fixtures.mjs
```
