#pragma once
#include "of-kernel-sys/cpp/ofk.h"

namespace ofk::probes {
struct Measurement;
rust::String kernel_version();
Measurement measure(uint8_t which, uint8_t variant, double scale);
} // namespace ofk::probes
