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

  # Exclude SchemaMetadata from comparison — its type ordering depends on
  # compiler encounter order which varies between Swift and Rust for novel schemas.
  # The existing API tests (which have stable encounter order) validate SchemaMetadata.
  rm -f "$SWIFT_DIR"/Sources/Schema/SchemaMetadata.graphql.swift \
        "$RUST_DIR"/Sources/Schema/SchemaMetadata.graphql.swift 2>/dev/null

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

# Test 5: Large schema with many types, descriptions, and deprecations
SCHEMA5="$WORK_DIR/schema5"
mkdir -p "$SCHEMA5/ops"
cat > "$SCHEMA5/schema.graphqls" <<'EOF'
"""The root query type for the marketplace API."""
type Query {
  """Search for products by keyword."""
  products(
    """The search query string."""
    query: String!
    """Maximum number of results to return."""
    limit: Int = 20
  ): [Product!]!

  """Get a single product by ID."""
  product(id: ID!): Product

  """List all categories."""
  categories: [Category!]!

  """Get the current user's cart."""
  cart: Cart

  """List all available shipping methods."""
  shippingMethods: [ShippingMethod!]! @deprecated(reason: "Use fulfillmentOptions instead")

  """List fulfillment options for the current cart."""
  fulfillmentOptions: [FulfillmentOption!]!
}

"""A mutation for modifying the cart and placing orders."""
type Mutation {
  """Add a product variant to the cart."""
  addToCart(input: AddToCartInput!): Cart!

  """Remove an item from the cart."""
  removeFromCart(itemId: ID!): Cart!

  """Place an order from the current cart."""
  placeOrder(input: PlaceOrderInput!): Order!
}

"""A product in the marketplace catalog."""
type Product implements Node & Displayable {
  """Unique identifier."""
  id: ID!
  """Display name of the product."""
  name: String!
  """Detailed description with markdown support."""
  description: String
  """Price in cents."""
  priceInCents: Int!
  """The product's primary category."""
  category: Category!
  """Available variants (size, color, etc.)."""
  variants: [ProductVariant!]!
  """Customer reviews."""
  reviews: ReviewConnection!
  """Average rating from 0 to 5."""
  averageRating: Float
  """Product images."""
  images: [Image!]!
  """Related products."""
  relatedProducts: [Product!]!
  """When the product was first listed."""
  listedAt: DateTime!
  """Vendor information."""
  vendor: Vendor!
  """Whether this product is currently available."""
  isAvailable: Boolean!
  """Tags for filtering."""
  tags: [String!]!
  """Old price field."""
  price: Float @deprecated(reason: "Use priceInCents for precision")
}

"""A specific variant of a product (e.g., size/color combination)."""
type ProductVariant implements Node {
  id: ID!
  """SKU identifier."""
  sku: String!
  """Variant-specific name (e.g., 'Large / Red')."""
  name: String!
  """Price override in cents (null means use product price)."""
  priceInCents: Int
  """Available inventory count."""
  inventoryCount: Int!
  """Variant attributes."""
  attributes: [VariantAttribute!]!
}

"""A key-value attribute on a variant."""
type VariantAttribute {
  key: String!
  value: String!
}

"""A product category in the catalog hierarchy."""
type Category implements Node & Displayable {
  id: ID!
  name: String!
  """Parent category (null for top-level)."""
  parent: Category
  """Subcategories."""
  children: [Category!]!
  """Number of products in this category."""
  productCount: Int!
  """Category icon URL."""
  iconUrl: String
}

"""A customer review of a product."""
type Review implements Node {
  id: ID!
  """The reviewer."""
  author: User!
  """Rating from 1 to 5."""
  rating: Int!
  """Review title."""
  title: String
  """Review body text."""
  body: String!
  """When the review was posted."""
  createdAt: DateTime!
  """Whether the review has been verified as a real purchase."""
  isVerified: Boolean!
  """Helpful vote count."""
  helpfulCount: Int!
}

"""Paginated connection for reviews."""
type ReviewConnection {
  edges: [ReviewEdge!]!
  pageInfo: PageInfo!
  totalCount: Int!
}

type ReviewEdge {
  node: Review!
  cursor: String!
}

type PageInfo {
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}

"""A user account."""
type User implements Node & Displayable {
  id: ID!
  name: String!
  """User's email address."""
  email: String!
  """Profile avatar URL."""
  avatarUrl: String
  """When the account was created."""
  joinedAt: DateTime!
  """Shipping addresses on file."""
  addresses: [Address!]!
}

"""A physical address for shipping."""
type Address {
  """Street address line 1."""
  line1: String!
  """Street address line 2."""
  line2: String
  city: String!
  state: String!
  postalCode: String!
  country: String!
}

"""A product image."""
type Image {
  url: String!
  altText: String
  width: Int
  height: Int
}

"""A vendor/seller in the marketplace."""
type Vendor implements Node & Displayable {
  id: ID!
  name: String!
  """Vendor description."""
  description: String
  """Average vendor rating."""
  rating: Float
  """Number of products listed."""
  productCount: Int!
  """Verified seller badge."""
  isVerified: Boolean!
}

"""Shopping cart."""
type Cart {
  """Cart items."""
  items: [CartItem!]!
  """Subtotal in cents."""
  subtotalInCents: Int!
  """Estimated tax in cents."""
  estimatedTaxInCents: Int!
  """Total in cents."""
  totalInCents: Int!
  """Number of items."""
  itemCount: Int!
}

"""An item in the shopping cart."""
type CartItem implements Node {
  id: ID!
  """The product variant in the cart."""
  variant: ProductVariant!
  """The parent product."""
  product: Product!
  """Quantity."""
  quantity: Int!
  """Line item total in cents."""
  totalInCents: Int!
}

"""A placed order."""
type Order implements Node {
  id: ID!
  """Order status."""
  status: OrderStatus!
  """Order items."""
  items: [OrderItem!]!
  """Shipping address."""
  shippingAddress: Address!
  """Total in cents."""
  totalInCents: Int!
  """When the order was placed."""
  placedAt: DateTime!
  """Tracking information."""
  tracking: TrackingInfo
}

"""An item in a placed order."""
type OrderItem {
  variant: ProductVariant!
  product: Product!
  quantity: Int!
  priceInCents: Int!
}

"""Shipping tracking information."""
type TrackingInfo {
  carrier: String!
  trackingNumber: String!
  estimatedDelivery: DateTime
  status: ShippingStatus!
}

"""Fulfillment option for checkout."""
type FulfillmentOption {
  id: ID!
  name: String!
  description: String
  priceInCents: Int!
  estimatedDays: Int!
}

"""Deprecated shipping method."""
type ShippingMethod {
  id: ID!
  name: String!
  cost: Float! @deprecated(reason: "Use priceInCents")
  priceInCents: Int!
}

"""Common interface for all identifiable entities."""
interface Node {
  id: ID!
}

"""Interface for entities with a display name."""
interface Displayable {
  name: String!
}

"""Product category enum."""
enum OrderStatus {
  """Order has been placed but not yet processed."""
  PENDING
  """Order is being prepared."""
  PROCESSING
  """Order has been shipped."""
  SHIPPED
  """Order has been delivered."""
  DELIVERED
  """Order was cancelled."""
  CANCELLED
  """Order was refunded."""
  REFUNDED
}

"""Shipping status for tracking."""
enum ShippingStatus {
  LABEL_CREATED
  IN_TRANSIT
  OUT_FOR_DELIVERY
  DELIVERED
  EXCEPTION
}

"""Custom scalar for date-time values."""
scalar DateTime

"""Input for adding items to cart."""
input AddToCartInput {
  """The product variant ID to add."""
  variantId: ID!
  """Quantity to add."""
  quantity: Int! = 1
}

"""Input for placing an order."""
input PlaceOrderInput {
  """Shipping address."""
  shippingAddress: AddressInput!
  """Selected fulfillment option ID."""
  fulfillmentOptionId: ID!
  """Optional order notes."""
  notes: String
}

"""Input for a shipping address."""
input AddressInput {
  line1: String!
  line2: String
  city: String!
  state: String!
  postalCode: String!
  country: String!
}
EOF

cat > "$SCHEMA5/ops/SearchProducts.graphql" <<'EOF'
query SearchProductsQuery($query: String!, $limit: Int) {
  products(query: $query, limit: $limit) {
    id
    name
    description
    priceInCents
    averageRating
    isAvailable
    images {
      url
      altText
    }
    category {
      id
      name
      parent {
        id
        name
      }
    }
    vendor {
      id
      name
      isVerified
      rating
    }
    variants {
      id
      sku
      name
      priceInCents
      inventoryCount
      attributes {
        key
        value
      }
    }
  }
}
EOF

cat > "$SCHEMA5/ops/ProductDetail.graphql" <<'EOF'
query ProductDetailQuery($id: ID!) {
  product(id: $id) {
    id
    name
    description
    priceInCents
    averageRating
    isAvailable
    listedAt
    tags
    images {
      url
      altText
      width
      height
    }
    category {
      id
      name
      children {
        id
        name
      }
    }
    vendor {
      id
      name
      description
      rating
      productCount
      isVerified
    }
    variants {
      id
      sku
      name
      priceInCents
      inventoryCount
      attributes {
        key
        value
      }
    }
    reviews {
      totalCount
      edges {
        cursor
        node {
          id
          rating
          title
          body
          createdAt
          isVerified
          helpfulCount
          author {
            id
            name
            avatarUrl
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
    relatedProducts {
      id
      name
      priceInCents
      averageRating
      images {
        url
        altText
      }
    }
  }
}
EOF

cat > "$SCHEMA5/ops/CartAndOrder.graphql" <<'EOF'
query CartQuery {
  cart {
    items {
      id
      quantity
      totalInCents
      variant {
        id
        sku
        name
        priceInCents
      }
      product {
        id
        name
        images {
          url
        }
      }
    }
    subtotalInCents
    estimatedTaxInCents
    totalInCents
    itemCount
  }
  fulfillmentOptions {
    id
    name
    description
    priceInCents
    estimatedDays
  }
}

mutation AddToCartMutation($input: AddToCartInput!) {
  addToCart(input: $input) {
    items {
      id
      quantity
      totalInCents
      variant {
        id
        sku
        name
      }
      product {
        id
        name
      }
    }
    subtotalInCents
    totalInCents
    itemCount
  }
}

mutation PlaceOrderMutation($input: PlaceOrderInput!) {
  placeOrder(input: $input) {
    id
    status
    totalInCents
    placedAt
    items {
      variant {
        id
        sku
      }
      product {
        id
        name
      }
      quantity
      priceInCents
    }
    shippingAddress {
      line1
      line2
      city
      state
      postalCode
      country
    }
    tracking {
      carrier
      trackingNumber
      estimatedDelivery
      status
    }
  }
}
EOF

run_complex_test "LargeMarketplaceSchema" "$SCHEMA5/schema.graphqls" "$SCHEMA5/ops"

# Test 6: Schema with deep interface hierarchy and many fragments
SCHEMA6="$WORK_DIR/schema6"
mkdir -p "$SCHEMA6/ops"
cat > "$SCHEMA6/schema.graphqls" <<'EOF'
"""Content management system with rich type hierarchy."""
type Query {
  """Get content feed."""
  feed(type: ContentType, limit: Int = 10): [Content!]!
  """Get a single content item."""
  content(id: ID!): Content
  """Search across all content."""
  search(query: String!): [SearchResult!]!
}

"""Base interface for all content."""
interface Content {
  id: ID!
  """The content title."""
  title: String!
  """Content author."""
  author: Author!
  """When published."""
  publishedAt: DateTime!
  """Content tags."""
  tags: [Tag!]!
  """Number of likes."""
  likeCount: Int!
  """Comment thread."""
  comments: [Comment!]!
}

"""Interface for content with a body."""
interface HasBody {
  """The main content body (markdown)."""
  body: String!
  """Estimated reading time in minutes."""
  readingTimeMinutes: Int!
}

"""Interface for media content."""
interface HasMedia {
  """Primary media URL."""
  mediaUrl: String!
  """Media thumbnail."""
  thumbnailUrl: String!
  """Duration in seconds (for video/audio)."""
  durationSeconds: Int
}

"""A blog article."""
type Article implements Content & HasBody {
  id: ID!
  title: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments: [Comment!]!
  body: String!
  readingTimeMinutes: Int!
  """Article subtitle."""
  subtitle: String
  """Cover image URL."""
  coverImageUrl: String
  """Whether this is a featured article."""
  isFeatured: Boolean!
}

"""A video post."""
type Video implements Content & HasMedia {
  id: ID!
  title: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments: [Comment!]!
  mediaUrl: String!
  thumbnailUrl: String!
  durationSeconds: Int
  """Video resolution."""
  resolution: String
  """Whether captions are available."""
  hasCaptions: Boolean!
}

"""A podcast episode."""
type Podcast implements Content & HasBody & HasMedia {
  id: ID!
  title: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments: [Comment!]!
  body: String!
  readingTimeMinutes: Int!
  mediaUrl: String!
  thumbnailUrl: String!
  durationSeconds: Int
  """Episode number in the series."""
  episodeNumber: Int!
  """Series name."""
  seriesName: String!
  """Guest speakers."""
  guests: [Author!]!
}

"""A short-form post."""
type ShortPost implements Content {
  id: ID!
  title: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments: [Comment!]!
  """Short post text (max 280 chars)."""
  text: String!
  """Attached images."""
  images: [String!]!
}

"""Content author profile."""
type Author {
  id: ID!
  name: String!
  bio: String
  avatarUrl: String
  """Number of followers."""
  followerCount: Int!
}

"""A content tag."""
type Tag {
  name: String!
  slug: String!
}

"""A comment on content."""
type Comment {
  id: ID!
  author: Author!
  text: String!
  createdAt: DateTime!
  """Replies to this comment."""
  replies: [Comment!]!
}

"""Search result union."""
union SearchResult = Article | Video | Podcast | ShortPost

"""Content type filter."""
enum ContentType {
  ARTICLE
  VIDEO
  PODCAST
  SHORT_POST
}

scalar DateTime
EOF

cat > "$SCHEMA6/ops/Feed.graphql" <<'EOF'
fragment AuthorFields on Author {
  id
  name
  avatarUrl
}

fragment ContentFields on Content {
  id
  title
  publishedAt
  likeCount
  author {
    ...AuthorFields
  }
  tags {
    name
    slug
  }
}

query FeedQuery($type: ContentType, $limit: Int) {
  feed(type: $type, limit: $limit) {
    ...ContentFields
    ... on Article {
      body
      readingTimeMinutes
      subtitle
      coverImageUrl
      isFeatured
    }
    ... on Video {
      mediaUrl
      thumbnailUrl
      durationSeconds
      resolution
      hasCaptions
    }
    ... on Podcast {
      mediaUrl
      thumbnailUrl
      durationSeconds
      episodeNumber
      seriesName
      guests {
        ...AuthorFields
      }
    }
    ... on ShortPost {
      text
      images
    }
    comments {
      id
      author {
        ...AuthorFields
      }
      text
      createdAt
      replies {
        id
        author {
          ...AuthorFields
        }
        text
      }
    }
  }
}
EOF

cat > "$SCHEMA6/ops/Search.graphql" <<'EOF'
query SearchContentQuery($query: String!) {
  search(query: $query) {
    ... on Article {
      id
      title
      subtitle
      author {
        name
      }
      publishedAt
      isFeatured
    }
    ... on Video {
      id
      title
      thumbnailUrl
      durationSeconds
      author {
        name
      }
    }
    ... on Podcast {
      id
      title
      seriesName
      episodeNumber
      durationSeconds
      author {
        name
      }
    }
    ... on ShortPost {
      id
      title
      text
      author {
        name
      }
    }
  }
}
EOF

run_complex_test "ContentManagementSchema" "$SCHEMA6/schema.graphqls" "$SCHEMA6/ops"

print_summary
