#include <stdint.h>

typedef struct Node Node;

struct Node {
    Node *next;
    int32_t value;
};

Node bump_node(Node node) {
    node.value += 1;
    return node;
}
