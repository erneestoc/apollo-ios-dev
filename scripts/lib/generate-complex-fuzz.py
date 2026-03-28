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


GENERATORS = [generate_case_0, generate_case_1]


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
