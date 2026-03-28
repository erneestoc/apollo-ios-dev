#!/usr/bin/env python3
"""Generate deterministic large complex GraphQL schemas for fuzz testing.

Creates schemas with:
- Multi-line and single-line descriptions/comments on types, fields, enums
- Deep interface hierarchies (3+ levels)
- Unions with many members
- Multiple fragments (including nested fragment spreads)
- Deep inline fragment nesting (3+ levels)
- @skip/@include conditional fields
- @deprecated fields and enum values
- Input objects with default values
- Custom scalars
- Lots of entity (composite) fields creating deep nesting

Each case is self-contained: schema.graphqls + operations/*.graphql + config.json

Usage:
    python3 generate-complex-fuzz.py <output_dir> <count>
"""

import json
import os
import sys


def generate_case_0(case_dir):
    """Large e-commerce schema with deep entity nesting and fragments."""
    schema = '''\
"""
The root query for the marketplace.
Provides access to products, orders, and user data.
"""
type Query {
  """Search products by keyword."""
  products(query: String!, limit: Int = 10): [Product!]!
  """Get product by unique ID."""
  product(id: ID!): Product
  """Current authenticated user."""
  viewer: User
  """Browse categories."""
  categories: [Category!]!
}

type Mutation {
  """Add item to the shopping cart."""
  addToCart(productId: ID!, quantity: Int! = 1): Cart!
  """Submit a product review."""
  submitReview(input: ReviewInput!): Review!
}

"A date-time string in ISO 8601 format."
scalar DateTime

"A URL string."
scalar URL

"""
A product available for purchase.
Products belong to categories and have variants.
"""
type Product implements Node & Searchable {
  "Unique product identifier."
  id: ID!
  "Product display name."
  name: String!
  """
  Long-form product description.
  Supports **markdown** formatting.
  """
  description: String
  "Price in cents to avoid floating point issues."
  priceInCents: Int!
  "The product category."
  category: Category!
  "Available variants (size, color)."
  variants: [Variant!]!
  "Product images."
  images: [Image!]!
  "Average customer rating (0-5)."
  rating: Float
  "When this product was listed."
  createdAt: DateTime!
  "The seller."
  vendor: Vendor!
  "Whether currently in stock."
  inStock: Boolean!
  "Related products you might like."
  related: [Product!]!
  "Customer reviews."
  reviews(first: Int = 5): ReviewConnection!
  "Old price field, use priceInCents instead."
  price: Float @deprecated(reason: "Use priceInCents for precision. Will be removed in v3.")
  "Searchable text content."
  searchText: String!
}

"""A product variant such as size or color."""
type Variant implements Node {
  id: ID!
  "SKU code."
  sku: String!
  "Variant label (e.g. 'Large / Blue')."
  label: String!
  "Additional price in cents (added to base)."
  additionalPriceInCents: Int!
  "Inventory count."
  stock: Int!
}

"""An image with dimensions."""
type Image {
  "Full-size image URL."
  url: URL!
  "Alt text for accessibility."
  alt: String
  "Width in pixels."
  width: Int
  "Height in pixels."
  height: Int
}

"""A product category in the catalog tree."""
type Category implements Node & Searchable {
  id: ID!
  name: String!
  "Parent category, null for top-level."
  parent: Category
  "Subcategories."
  children: [Category!]!
  "Products in this category."
  productCount: Int!
  "URL-friendly slug."
  slug: String!
  searchText: String!
}

"""A marketplace vendor."""
type Vendor implements Node & Searchable {
  id: ID!
  name: String!
  "Company description."
  bio: String
  "Seller rating."
  rating: Float
  "Total products listed."
  productCount: Int!
  "Verified seller status."
  verified: Boolean!
  searchText: String!
}

"""An authenticated user."""
type User implements Node {
  id: ID!
  "Display name."
  name: String!
  "Email address."
  email: String!
  "Profile picture."
  avatarUrl: URL
  "Shipping addresses."
  addresses: [Address!]!
  "Shopping cart."
  cart: Cart
  "Order history."
  orders(first: Int = 10): [Order!]!
  "Account creation date."
  joinedAt: DateTime!
}

"""A shipping address."""
type Address {
  line1: String!
  line2: String
  city: String!
  state: String!
  zip: String!
  country: String!
}

"""Shopping cart."""
type Cart {
  "Cart line items."
  items: [CartItem!]!
  "Subtotal in cents."
  subtotal: Int!
  "Item count."
  itemCount: Int!
}

"""A single cart line item."""
type CartItem implements Node {
  id: ID!
  product: Product!
  variant: Variant
  quantity: Int!
  "Line total in cents."
  lineTotal: Int!
}

"""A customer order."""
type Order implements Node {
  id: ID!
  "Order status."
  status: OrderStatus!
  "Order items."
  items: [OrderItem!]!
  "Shipping address."
  shippingAddress: Address!
  "Total in cents."
  total: Int!
  "When placed."
  placedAt: DateTime!
  "Tracking info."
  tracking: Tracking
}

"""An item in an order."""
type OrderItem {
  product: Product!
  variant: Variant
  quantity: Int!
  priceInCents: Int!
}

"""Shipment tracking information."""
type Tracking {
  carrier: String!
  trackingNumber: String!
  estimatedDelivery: DateTime
  status: ShippingStatus!
}

"""A product review."""
type Review implements Node {
  id: ID!
  "The reviewer."
  author: User!
  "Star rating 1-5."
  rating: Int!
  "Review title."
  title: String
  "Review body."
  body: String!
  "When posted."
  createdAt: DateTime!
  "Verified purchase."
  verified: Boolean!
}

"""Paginated review connection."""
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

"""Common node interface."""
interface Node {
  id: ID!
}

"""Searchable content interface."""
interface Searchable {
  "Text content for full-text search."
  searchText: String!
}

"""Order status enum."""
enum OrderStatus {
  "Awaiting processing."
  PENDING
  "Being prepared."
  PROCESSING
  "Shipped to customer."
  SHIPPED
  "Successfully delivered."
  DELIVERED
  "Order was cancelled."
  CANCELLED
}

"""Shipping carrier status."""
enum ShippingStatus {
  LABEL_CREATED
  IN_TRANSIT
  OUT_FOR_DELIVERY
  DELIVERED
  EXCEPTION
}

"""Input for submitting a review."""
input ReviewInput {
  productId: ID!
  rating: Int!
  title: String
  body: String!
}
'''

    ops = []

    # Operation 1: Complex product search with deep nesting
    ops.append('''\
fragment ImageFields on Image {
  url
  alt
  width
  height
}

fragment VendorSummary on Vendor {
  id
  name
  rating
  verified
}

fragment ProductCard on Product {
  id
  name
  priceInCents
  rating
  inStock
  images {
    ...ImageFields
  }
  vendor {
    ...VendorSummary
  }
  category {
    id
    name
    slug
  }
}

query SearchProductsQuery($query: String!, $limit: Int) {
  products(query: $query, limit: $limit) {
    ...ProductCard
    description
    variants {
      id
      sku
      label
      additionalPriceInCents
      stock
    }
    reviews(first: 3) {
      totalCount
      edges {
        node {
          id
          rating
          title
          body
          author {
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
    related {
      ...ProductCard
    }
  }
}
''')

    # Operation 2: Viewer with conditional fields and deep nesting
    ops.append('''\
query ViewerDashboardQuery($includeOrders: Boolean!, $includeCart: Boolean!, $skipAddresses: Boolean!) {
  viewer {
    id
    name
    email
    avatarUrl
    joinedAt
    addresses @skip(if: $skipAddresses) {
      line1
      line2
      city
      state
      zip
      country
    }
    cart @include(if: $includeCart) {
      items {
        id
        quantity
        lineTotal
        product {
          id
          name
          priceInCents
          images {
            url
            alt
          }
        }
        variant {
          id
          sku
          label
        }
      }
      subtotal
      itemCount
    }
    orders(first: 5) @include(if: $includeOrders) {
      id
      status
      total
      placedAt
      items {
        product {
          id
          name
        }
        quantity
        priceInCents
      }
      shippingAddress {
        city
        state
        country
      }
      tracking {
        carrier
        trackingNumber
        status
        estimatedDelivery
      }
    }
  }
}
''')

    # Operation 3: Mutation with fragments
    ops.append('''\
fragment ReviewFields on Review {
  id
  rating
  title
  body
  createdAt
  verified
  author {
    id
    name
  }
}

mutation SubmitReviewMutation($input: ReviewInput!) {
  submitReview(input: $input) {
    ...ReviewFields
  }
}
''')

    # Operation 4: Categories with recursive-like nesting
    ops.append('''\
query CategoriesQuery {
  categories {
    id
    name
    slug
    productCount
    children {
      id
      name
      slug
      productCount
      children {
        id
        name
        slug
        productCount
      }
    }
  }
}
''')

    write_case(case_dir, "MarketplaceAPI", schema, ops)


def generate_case_1(case_dir):
    """CMS schema with deep interface hierarchy and union search."""
    schema = '''\
"""
Content management system API.
Supports articles, videos, podcasts, and galleries.
"""
type Query {
  """
  Get the content feed.
  Returns a mixed list of content types ordered by publish date.
  """
  feed(limit: Int = 20, offset: Int = 0): [Content!]!
  "Search across all content types."
  search(query: String!): [SearchResult!]!
  "Get a single content item by ID."
  content(id: ID!): Content
  "Get content by tag."
  byTag(tag: String!, limit: Int = 10): [Content!]!
}

"ISO 8601 date-time."
scalar DateTime

"""
Base interface for all publishable content.
Every content type must implement this interface.
"""
interface Content {
  "Unique content identifier."
  id: ID!
  "Content title."
  title: String!
  "URL-friendly slug."
  slug: String!
  "Content author."
  author: Author!
  "Publication date."
  publishedAt: DateTime!
  "Content tags for categorization."
  tags: [Tag!]!
  "Number of likes."
  likeCount: Int!
  "Comment thread."
  comments(first: Int = 10): [Comment!]!
  "Whether this content is featured on the homepage."
  featured: Boolean!
}

"""
Interface for content with a text body.
Articles and podcasts have show notes.
"""
interface Readable implements Content {
  id: ID!
  title: String!
  slug: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments(first: Int = 10): [Comment!]!
  featured: Boolean!
  "The main body text (markdown)."
  body: String!
  "Estimated reading time in minutes."
  readingTime: Int!
}

"""Interface for content with media attachments."""
interface MediaContent implements Content {
  id: ID!
  title: String!
  slug: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments(first: Int = 10): [Comment!]!
  featured: Boolean!
  "Primary media URL."
  mediaUrl: String!
  "Thumbnail image URL."
  thumbnailUrl: String!
  "Duration in seconds."
  durationSeconds: Int
}

"""
A long-form blog article.
Implements both Content and Readable.
"""
type Article implements Content & Readable {
  id: ID!
  title: String!
  slug: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments(first: Int = 10): [Comment!]!
  featured: Boolean!
  body: String!
  readingTime: Int!
  "Subtitle or deck."
  subtitle: String
  "Hero image URL."
  coverImageUrl: String
  "Article series name."
  series: String
}

"""A video post."""
type Video implements Content & MediaContent {
  id: ID!
  title: String!
  slug: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments(first: Int = 10): [Comment!]!
  featured: Boolean!
  mediaUrl: String!
  thumbnailUrl: String!
  durationSeconds: Int
  "Video resolution (e.g., '1080p')."
  resolution: String
  "Whether captions are available."
  hasCaptions: Boolean!
}

"""A podcast episode with show notes."""
type Podcast implements Content & Readable & MediaContent {
  id: ID!
  title: String!
  slug: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments(first: Int = 10): [Comment!]!
  featured: Boolean!
  body: String!
  readingTime: Int!
  mediaUrl: String!
  thumbnailUrl: String!
  durationSeconds: Int
  "Episode number."
  episodeNumber: Int!
  "Podcast series name."
  seriesName: String!
  "Guest speakers."
  guests: [Author!]!
}

"""A photo gallery."""
type Gallery implements Content & MediaContent {
  id: ID!
  title: String!
  slug: String!
  author: Author!
  publishedAt: DateTime!
  tags: [Tag!]!
  likeCount: Int!
  comments(first: Int = 10): [Comment!]!
  featured: Boolean!
  mediaUrl: String!
  thumbnailUrl: String!
  durationSeconds: Int
  "Gallery photos."
  photos: [Photo!]!
  "Total photo count."
  photoCount: Int!
}

"""A photo in a gallery."""
type Photo {
  url: String!
  caption: String
  width: Int!
  height: Int!
}

"""Content author."""
type Author {
  id: ID!
  name: String!
  bio: String
  avatarUrl: String
  "Number of followers."
  followerCount: Int!
}

"A content tag."
type Tag {
  name: String!
  slug: String!
}

"""A comment on any content."""
type Comment {
  id: ID!
  author: Author!
  text: String!
  createdAt: DateTime!
  "Nested replies."
  replies: [Comment!]!
  likeCount: Int!
}

"""Search result union across all content types."""
union SearchResult = Article | Video | Podcast | Gallery

"Content type filter."
enum ContentType {
  ARTICLE
  VIDEO
  PODCAST
  GALLERY
  "Deprecated: use GALLERY instead."
  PHOTO_SET @deprecated(reason: "Renamed to GALLERY in v2.")
}
'''

    ops = []

    # Complex feed query with fragments on interfaces and concrete types
    ops.append('''\
fragment AuthorCard on Author {
  id
  name
  avatarUrl
  followerCount
}

fragment TagList on Tag {
  name
  slug
}

fragment ContentBase on Content {
  id
  title
  slug
  publishedAt
  likeCount
  featured
  author {
    ...AuthorCard
  }
  tags {
    ...TagList
  }
}

fragment CommentThread on Comment {
  id
  author {
    ...AuthorCard
  }
  text
  createdAt
  likeCount
  replies {
    id
    author {
      name
    }
    text
    createdAt
  }
}

query FeedQuery($limit: Int, $offset: Int) {
  feed(limit: $limit, offset: $offset) {
    ...ContentBase
    ... on Article {
      body
      readingTime
      subtitle
      coverImageUrl
      series
    }
    ... on Video {
      mediaUrl
      thumbnailUrl
      durationSeconds
      resolution
      hasCaptions
    }
    ... on Podcast {
      body
      readingTime
      mediaUrl
      thumbnailUrl
      durationSeconds
      episodeNumber
      seriesName
      guests {
        ...AuthorCard
      }
    }
    ... on Gallery {
      mediaUrl
      thumbnailUrl
      photos {
        url
        caption
        width
        height
      }
      photoCount
    }
    comments(first: 3) {
      ...CommentThread
    }
  }
}
''')

    # Search across union types
    ops.append('''\
query SearchQuery($query: String!) {
  search(query: $query) {
    ... on Article {
      id
      title
      subtitle
      slug
      author {
        name
      }
      publishedAt
      readingTime
      featured
    }
    ... on Video {
      id
      title
      slug
      thumbnailUrl
      durationSeconds
      resolution
      author {
        name
      }
    }
    ... on Podcast {
      id
      title
      slug
      seriesName
      episodeNumber
      durationSeconds
      author {
        name
      }
      guests {
        name
      }
    }
    ... on Gallery {
      id
      title
      slug
      photoCount
      thumbnailUrl
      author {
        name
      }
    }
  }
}
''')

    write_case(case_dir, "ContentAPI", schema, ops)


def write_case(case_dir, namespace, schema, operations):
    """Write a test case to disk."""
    ops_dir = os.path.join(case_dir, "operations")
    os.makedirs(ops_dir, exist_ok=True)

    schema_path = os.path.join(case_dir, "schema.graphqls")
    with open(schema_path, "w") as f:
        f.write(schema)

    for i, op in enumerate(operations):
        op_path = os.path.join(ops_dir, f"op-{i:03d}.graphql")
        with open(op_path, "w") as f:
            f.write(op)

    config = {
        "schemaNamespace": namespace,
        "input": {
            "schemaSearchPaths": [schema_path],
            "operationSearchPaths": [os.path.join(ops_dir, "*.graphql")],
        },
        "output": {
            "testMocks": {"none": {}},
            "schemaTypes": {
                "path": os.path.join(case_dir, "Generated"),
                "moduleType": {"swiftPackageManager": {}},
            },
            "operations": {"inSchemaModule": {}},
        },
        "options": {
            "schemaDocumentation": "include",
            "pruneGeneratedFiles": False,
        },
    }

    config_path = os.path.join(case_dir, "config.json")
    with open(config_path, "w") as f:
        json.dump(config, f, indent=2)
        f.write("\n")


def generate_case_2(case_dir):
    """Issue 1: Multi-argument fields for operationIdentifier hash testing."""
    schema = '''\
type Query {
  user(id: ID!, name: String): User
  search(query: String!, limit: Int, offset: Int, filter: SearchFilter): [Result!]!
  node(id: ID!): Node
}

type User implements Node {
  id: ID!
  name: String!
  email: String!
  posts(first: Int, after: String, orderBy: PostOrder): PostConnection!
}

type Post implements Node {
  id: ID!
  title: String!
  body: String!
  author: User!
  comments(first: Int, after: String): [Comment!]!
}

type Comment implements Node {
  id: ID!
  text: String!
  author: User!
}

type PostConnection {
  edges: [PostEdge!]!
  pageInfo: PageInfo!
}

type PostEdge {
  node: Post!
  cursor: String!
}

type PageInfo {
  hasNextPage: Boolean!
  endCursor: String
}

type Result {
  id: ID!
  title: String!
  score: Float!
}

interface Node {
  id: ID!
}

input SearchFilter {
  category: String
  minScore: Float
}

enum PostOrder {
  NEWEST
  OLDEST
  TOP
}
'''

    ops = []
    # Query with multiple variable-only field arguments
    ops.append('''\
query UserPostsQuery($userId: ID!, $first: Int, $after: String, $orderBy: PostOrder) {
  user(id: $userId) {
    id
    name
    email
    posts(first: $first, after: $after, orderBy: $orderBy) {
      edges {
        node {
          id
          title
          body
          comments(first: 3) {
            id
            text
            author {
              name
            }
          }
        }
        cursor
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}
''')

    # Query with inline literal arguments
    ops.append('''\
query SearchQuery($query: String!, $limit: Int, $offset: Int) {
  search(query: $query, limit: $limit, offset: $offset, filter: {category: "tech", minScore: 0.5}) {
    id
    title
    score
  }
}
''')

    write_case(case_dir, "MultiArgAPI", schema, ops)


def generate_case_3(case_dir):
    """Issue 3: Cross-union field merging and sibling accessors."""
    schema = '''\
type Query {
  feed: [FeedItem!]!
}

union FeedItem = TextPost | ImagePost | VideoPost | Poll

type TextPost {
  id: ID!
  title: String!
  body: String!
  author: Author!
  likes: Int!
}

type ImagePost {
  id: ID!
  title: String!
  imageUrl: String!
  caption: String
  author: Author!
  likes: Int!
}

type VideoPost {
  id: ID!
  title: String!
  videoUrl: String!
  duration: Int!
  author: Author!
  likes: Int!
}

type Poll {
  id: ID!
  question: String!
  options: [PollOption!]!
  author: Author!
  likes: Int!
}

type PollOption {
  text: String!
  votes: Int!
}

type Author {
  id: ID!
  name: String!
  avatar: String
}
'''

    ops = []
    # Union query — each branch selects shared fields (id, title, author, likes)
    # plus type-specific fields. Tests sibling accessor merging.
    ops.append('''\
query FeedQuery {
  feed {
    ... on TextPost {
      id
      title
      body
      author {
        id
        name
        avatar
      }
      likes
    }
    ... on ImagePost {
      id
      title
      imageUrl
      caption
      author {
        id
        name
      }
      likes
    }
    ... on VideoPost {
      id
      title
      videoUrl
      duration
      author {
        id
        name
      }
      likes
    }
    ... on Poll {
      id
      question
      options {
        text
        votes
      }
      author {
        id
        name
      }
      likes
    }
  }
}
''')

    write_case(case_dir, "UnionMergeAPI", schema, ops)


def generate_case_4(case_dir):
    """Issue 4: Fragment spread with nested entity types (typealias merging)."""
    schema = '''\
type Query {
  hero: Character
}

interface Character {
  id: ID!
  name: String!
  friends: [Character!]!
  homeWorld: Planet!
}

type Human implements Character {
  id: ID!
  name: String!
  friends: [Character!]!
  homeWorld: Planet!
  height: Float
}

type Droid implements Character {
  id: ID!
  name: String!
  friends: [Character!]!
  homeWorld: Planet!
  primaryFunction: String
}

type Planet {
  id: ID!
  name: String!
  climate: String
  population: Int
}
'''

    ops = []
    # Fragment with nested entity type — tests typealias generation
    ops.append('''\
fragment CharacterFields on Character {
  id
  name
  homeWorld {
    id
    name
    climate
  }
  friends {
    id
    name
  }
}

query HeroQuery {
  hero {
    ...CharacterFields
    ... on Human {
      height
      homeWorld {
        population
      }
    }
    ... on Droid {
      primaryFunction
    }
  }
}
''')

    write_case(case_dir, "TypealiasAPI", schema, ops)


def generate_case_5(case_dir):
    """Issue 5: Variable default values — Bool, Int, null, enum."""
    schema = '''\
type Query {
  items(
    includeHidden: Boolean! = false
    limit: Int! = 10
    filter: ItemFilter
    sortBy: SortOrder = NEWEST
  ): [Item!]!
}

type Item {
  id: ID!
  name: String!
  hidden: Boolean!
}

input ItemFilter {
  category: String = null
  minPrice: Int = 0
  active: Boolean = true
}

enum SortOrder {
  NEWEST
  OLDEST
  ALPHABETICAL
}
'''

    ops = []
    ops.append('''\
query ItemsQuery(
  $includeHidden: Boolean! = false
  $limit: Int! = 10
  $filter: ItemFilter = null
  $sortBy: SortOrder = NEWEST
) {
  items(
    includeHidden: $includeHidden
    limit: $limit
    filter: $filter
    sortBy: $sortBy
  ) {
    id
    name
    hidden
  }
}
''')

    write_case(case_dir, "DefaultValuesAPI", schema, ops)


def generate_case_6(case_dir):
    """Issue 6: Same-type inline fragment (should be absorbed, not wrapper)."""
    schema = '''\
type Query {
  allAnimals: [Animal!]!
}

interface Animal {
  id: ID!
  species: String!
  weight: Float!
}

interface Pet implements Animal {
  id: ID!
  species: String!
  weight: Float!
  name: String!
  owner: Human!
}

type Dog implements Animal & Pet {
  id: ID!
  species: String!
  weight: Float!
  name: String!
  owner: Human!
  breed: String!
}

type Cat implements Animal & Pet {
  id: ID!
  species: String!
  weight: Float!
  name: String!
  owner: Human!
  indoor: Boolean!
}

type Human {
  id: ID!
  firstName: String!
}
'''

    ops = []
    # Inline fragment matching parent type should be absorbed
    ops.append('''\
query AllAnimalsQuery {
  allAnimals {
    id
    species
    ... on Animal {
      weight
    }
    ... on Pet {
      name
      owner {
        firstName
      }
    }
    ... on Dog {
      breed
    }
    ... on Cat {
      indoor
    }
  }
}
''')

    write_case(case_dir, "AbsorbedInlineAPI", schema, ops)


def generate_case_7(case_dir):
    """Issue 7: fulfilledFragments — fragment ObjectIdentifier inclusion."""
    schema = '''\
type Query {
  node(id: ID!): Node
}

interface Node {
  id: ID!
}

interface Displayable {
  title: String!
}

type Article implements Node & Displayable {
  id: ID!
  title: String!
  body: String!
}

type Image implements Node & Displayable {
  id: ID!
  title: String!
  url: String!
  width: Int!
  height: Int!
}
'''

    ops = []
    ops.append('''\
fragment NodeFields on Node {
  id
}

fragment DisplayFields on Displayable {
  title
}

query NodeQuery($id: ID!) {
  node(id: $id) {
    ...NodeFields
    ...DisplayFields
    ... on Article {
      body
    }
    ... on Image {
      url
      width
      height
    }
  }
}
''')

    write_case(case_dir, "FulfilledFragsAPI", schema, ops)


def generate_case_8(case_dir):
    """Issue 8: _SelectionSet disambiguation with conflicting field names."""
    schema = '''\
type Query {
  result: OperationResult!
}

type OperationResult {
  success: Boolean!
  "A field named 'errors' whose type will generate a struct named 'Error'"
  errors: [Error!]!
  "A field named 'string' whose type will generate a struct named 'String'"
  string: StringWrapper
  data: DataPayload
}

type Error {
  code: Int!
  message: String!
}

type StringWrapper {
  value: String!
  length: Int!
}

type DataPayload {
  id: ID!
  content: String!
}
'''

    ops = []
    ops.append('''\
query ResultQuery {
  result {
    success
    errors {
      code
      message
    }
    string {
      value
      length
    }
    data {
      id
      content
    }
  }
}
''')

    write_case(case_dir, "DisambiguationAPI", schema, ops)


GENERATORS = [
    generate_case_0, generate_case_1, generate_case_2, generate_case_3,
    generate_case_4, generate_case_5, generate_case_6, generate_case_7,
    generate_case_8,
]


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <output_dir> [count]", file=sys.stderr)
        sys.exit(1)

    output_dir = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) > 2 else len(GENERATORS)
    os.makedirs(output_dir, exist_ok=True)

    generated = 0
    for i in range(min(count, len(GENERATORS))):
        case_dir = os.path.join(output_dir, f"case-{i:04d}")
        GENERATORS[i](case_dir)
        generated += 1
        print(f"  [{i+1}/{count}] {case_dir}")

    print(f"\nGenerated {generated} deterministic complex cases.")


if __name__ == "__main__":
    main()
