#include <string>

#if defined(PROBE_DLL_EXPORT)
#define PROBE_API __declspec(dllexport)
#elif defined(PROBE_DLL_IMPORT)
#define PROBE_API __declspec(dllimport)
#else
#define PROBE_API
#endif

namespace PROBE_NAMESPACE {
class PROBE_API Base {
  public:
    virtual ~Base();
    virtual std::string label() const = 0;
};

class PROBE_API Value final : public Base {
  public:
    explicit Value(int number);
    std::string label() const override;
    int fail() const;

  private:
    int number;
};

PROBE_API Base *create();
PROBE_API void destroy(Base *value);
} // namespace PROBE_NAMESPACE

#undef PROBE_API
