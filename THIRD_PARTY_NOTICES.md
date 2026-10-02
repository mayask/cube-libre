# Third-party notices

GAN Gen2/Gen3/Gen4 packet layouts, read-request framing, AES salting/overlap
scheme and cubie-to-facelet maps in `crates/cube-core` are adapted from
[afedotov/gan-web-bluetooth](https://github.com/afedotov/gan-web-bluetooth).
The Gen1 UUIDs, firmware/key derivation, ECB overlap, facelet packing and polling
reference are adapted from the MIT fork
[poliva/smartcube-web-bluetooth](https://github.com/poliva/smartcube-web-bluetooth);
its packet validation was also consulted. Both original notices are preserved
below. See [docs/GAN_PROTOCOLS.md](docs/GAN_PROTOCOLS.md) for pinned reference
revisions and adaptation boundaries.

Thank you to Andy Fedotov and Pau Oliva for sharing their implementations, and
to Chen Shuang / csTimer for protocol reverse engineering acknowledged by the
upstreams. No csTimer source code is vendored in this project.

## gan-web-bluetooth — MIT License

Copyright (c) Andy Fedotov, https://github.com/afedotov

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## smartcube-web-bluetooth — MIT License

Copyright (c) Pau Oliva, https://github.com/poliva
Copyright (c) Andy Fedotov, https://github.com/afedotov

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Other dependencies

Dioxus: MIT / Apache-2.0. btleplug: MIT / Apache-2.0 / BSD-3-Clause.
RustCrypto AES: MIT / Apache-2.0. Consult the locked crates for their complete
license texts. This project is independent of GAN and CubeStation.
