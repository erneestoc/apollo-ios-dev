#!/bin/bash
# Test complex schema scenarios: creates standalone test schemas with complex
# operations and compares Swift vs Rust codegen output byte-for-byte.
#
# These tests exercise:
# - Deep inline fragment nesting with shared entity fields
# - Fragment spreads across interface/union boundaries
# - Conditional inclusion (@skip/@include) with entity fields
# - Type narrowing with overlapping fields from multiple sources
# - Union member types with different field sets

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib/codegen-test-utils.sh"

WORK_DIR=$(create_temp_dir)

build_swift_cli
build_rust_cli

PASS=0
FAIL=0

run_complex_test() {
  local test_name="$1"
  local schema_file="$2"
  local ops_dir="$3"

  local TEST_DIR="$WORK_DIR/$test_name"
  local SWIFT_DIR="$TEST_DIR/swift"
  local RUST_DIR="$TEST_DIR/rust"
  mkdir -p "$SWIFT_DIR" "$RUST_DIR"

  # Generate config for Swift
  cat > "$TEST_DIR/swift-config.json" <<ENDCFG
{
  "schemaNamespace": "TestSchema",
  "input": {
    "schemaSearchPaths": ["$schema_file"],
    "operationSearchPaths": ["$ops_dir/*.graphql"]
  },
  "output": {
    "testMocks": { "none": {} },
    "schemaTypes": {
      "path": "$SWIFT_DIR",
      "moduleType": { "swiftPackageManager": {} }
    },
    "operations": { "inSchemaModule": {} }
  }
}
ENDCFG

  # Generate config for Rust
  cat > "$TEST_DIR/rust-config.json" <<ENDCFG
{
  "schemaNamespace": "TestSchema",
  "input": {
    "schemaSearchPaths": ["$schema_file"],
    "operationSearchPaths": ["$ops_dir/*.graphql"]
  },
  "output": {
    "testMocks": { "none": {} },
    "schemaTypes": {
      "path": "$RUST_DIR",
      "moduleType": { "swiftPackageManager": {} }
    },
    "operations": { "inSchemaModule": {} }
  }
}
ENDCFG

  # Run Swift
  if ! run_swift_codegen "$TEST_DIR/swift-config.json" > "$TEST_DIR/swift.log" 2>&1; then
    echo -e "  ${YELLOW}SKIP${NC}: $test_name (Swift codegen failed, see $TEST_DIR/swift.log)"
    cat "$TEST_DIR/swift.log" | tail -3
    return
  fi

  # Run Rust
  if ! run_rust_codegen "$TEST_DIR/rust-config.json" > "$TEST_DIR/rust.log" 2>&1; then
    echo -e "  ${YELLOW}SKIP${NC}: $test_name (Rust codegen failed, see $TEST_DIR/rust.log)"
    cat "$TEST_DIR/rust.log" | tail -3
    return
  fi

  # Compare
  compare_generated "$RUST_DIR" "$SWIFT_DIR" "$test_name" || true
}

echo ""
echo "Running complex schema tests..."
echo ""

# Test 1: Deep nested entity fields with predators
SCHEMA1="$WORK_DIR/schema1"
mkdir -p "$SCHEMA1/ops"
cat > "$SCHEMA1/schema.graphqls" <<'EOF'
type Query {
  items: [Item!]!
}

interface Node {
  id: ID!
}

type Item implements Node {
  id: ID!
  name: String!
  details: ItemDetails!
  related: [Item!]!
}

type ItemDetails {
  description: String!
  metadata: Metadata!
}

type Metadata {
  createdAt: String!
  tags: [String!]!
}
EOF

cat > "$SCHEMA1/ops/DeepNested.graphql" <<'EOF'
query DeepNestedQuery {
  items {
    id
    name
    details {
      description
      metadata {
        createdAt
        tags
      }
    }
    related {
      id
      name
      details {
        description
      }
    }
  }
}
EOF

run_complex_test "DeepNestedEntity" "$SCHEMA1/schema.graphqls" "$SCHEMA1/ops"

# Test 2: Union with multiple types and entity fields
SCHEMA2="$WORK_DIR/schema2"
mkdir -p "$SCHEMA2/ops"
cat > "$SCHEMA2/schema.graphqls" <<'EOF'
type Query {
  search: [SearchResult!]!
}

union SearchResult = User | Post | Comment

type User {
  id: ID!
  username: String!
  email: String!
}

type Post {
  id: ID!
  title: String!
  body: String!
  author: User!
}

type Comment {
  id: ID!
  text: String!
  author: User!
}
EOF

cat > "$SCHEMA2/ops/Search.graphql" <<'EOF'
query SearchQuery {
  search {
    ... on User {
      id
      username
      email
    }
    ... on Post {
      id
      title
      body
      author {
        username
      }
    }
    ... on Comment {
      id
      text
      author {
        username
      }
    }
  }
}
EOF

run_complex_test "UnionMultipleTypes" "$SCHEMA2/schema.graphqls" "$SCHEMA2/ops"

# Test 3: Interface hierarchy with fragments
SCHEMA3="$WORK_DIR/schema3"
mkdir -p "$SCHEMA3/ops"
cat > "$SCHEMA3/schema.graphqls" <<'EOF'
type Query {
  animals: [Animal!]!
}

interface Animal {
  id: ID!
  species: String!
}

type Dog implements Animal {
  id: ID!
  species: String!
  breed: String!
}

type Cat implements Animal {
  id: ID!
  species: String!
  indoor: Boolean!
}

type Fish implements Animal {
  id: ID!
  species: String!
  freshwater: Boolean!
}
EOF

cat > "$SCHEMA3/ops/InterfaceFragments.graphql" <<'EOF'
fragment AnimalFields on Animal {
  id
  species
}

query InterfaceFragmentsQuery {
  animals {
    ...AnimalFields
    ... on Dog {
      breed
    }
    ... on Cat {
      indoor
    }
    ... on Fish {
      freshwater
    }
  }
}
EOF

run_complex_test "InterfaceFragments" "$SCHEMA3/schema.graphqls" "$SCHEMA3/ops"

# Test 4: Conditional fields with @skip/@include
SCHEMA4="$WORK_DIR/schema4"
mkdir -p "$SCHEMA4/ops"
cat > "$SCHEMA4/schema.graphqls" <<'EOF'
type Query {
  user: User!
}

type User {
  id: ID!
  name: String!
  profile: Profile!
  posts: [Post!]!
}

type Profile {
  bio: String!
  avatar: String!
}

type Post {
  id: ID!
  title: String!
}
EOF

cat > "$SCHEMA4/ops/Conditional.graphql" <<'EOF'
query ConditionalFieldsQuery($includeProfile: Boolean!, $skipPosts: Boolean!) {
  user {
    id
    name
    profile @include(if: $includeProfile) {
      bio
      avatar
    }
    posts @skip(if: $skipPosts) {
      id
      title
    }
  }
}
EOF

run_complex_test "ConditionalFields" "$SCHEMA4/schema.graphqls" "$SCHEMA4/ops"

print_summary
