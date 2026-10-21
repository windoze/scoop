#include <array>
extern "C" int m52_answer(void) {
    constexpr std::array<int, 2> values{20, 22};
    return values[0] + values[1];
}
