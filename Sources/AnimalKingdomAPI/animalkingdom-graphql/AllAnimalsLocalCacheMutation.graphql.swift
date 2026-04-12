// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class AllAnimalsLocalCacheMutation: LocalCacheMutation {
  public static let operationType: GraphQLOperationType = .query

  public init() {}

  public struct Data: TestSchema.MutableSelectionSet {
    public var __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Query }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("allAnimals", [AllAnimal].self),
    ] }

    public var allAnimals: [AllAnimal] {
      get { __data["allAnimals"] }
      set { __data["allAnimals"] = newValue }
    }

    public init(
      allAnimals: [AllAnimal]
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Query.typename,
          "allAnimals": allAnimals._fieldData,
        ],
        fulfilledFragments: [
          ObjectIdentifier(AllAnimalsLocalCacheMutation.Data.self)
        ]
      ))
    }

    /// AllAnimal
    public struct AllAnimal: TestSchema.MutableSelectionSet {
      public var __data: DataDict
      public init(_dataDict: DataDict) { __data = _dataDict }

      public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
      public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("species", String.self),
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .inlineFragment(AsBird.self),
      ] }

      public var species: String {
        get { __data["species"] }
        set { __data["species"] = newValue }
      }
      public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? {
        get { __data["skinCovering"] }
        set { __data["skinCovering"] = newValue }
      }

      public var asBird: AsBird? {
        get { _asInlineFragment() }
        set { if let newData = newValue?.__data._data { __data._data = newData }}
      }

      public init(
        __typename: String,
        species: String,
        skinCovering: GraphQLEnum<TestSchema.SkinCovering>? = nil
      ) {
        self.init(_dataDict: DataDict(
          data: [
            "__typename": __typename,
            "species": species,
            "skinCovering": skinCovering,
          ],
          fulfilledFragments: [
            ObjectIdentifier(AllAnimalsLocalCacheMutation.Data.AllAnimal.self)
          ]
        ))
      }

      /// AllAnimal.AsBird
      public struct AsBird: TestSchema.MutableInlineFragment {
        public var __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = AllAnimalsLocalCacheMutation.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Bird }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("wingspan", Double.self),
        ] }

        public var wingspan: Double {
          get { __data["wingspan"] }
          set { __data["wingspan"] = newValue }
        }
        public var species: String {
          get { __data["species"] }
          set { __data["species"] = newValue }
        }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? {
          get { __data["skinCovering"] }
          set { __data["skinCovering"] = newValue }
        }

        public init(
          wingspan: Double,
          species: String,
          skinCovering: GraphQLEnum<TestSchema.SkinCovering>? = nil
        ) {
          self.init(_dataDict: DataDict(
            data: [
              "__typename": TestSchema.Objects.Bird.typename,
              "wingspan": wingspan,
              "species": species,
              "skinCovering": skinCovering,
            ],
            fulfilledFragments: [
              ObjectIdentifier(AllAnimalsLocalCacheMutation.Data.AllAnimal.self),
              ObjectIdentifier(AllAnimalsLocalCacheMutation.Data.AllAnimal.AsBird.self)
            ]
          ))
        }
      }
    }
  }
}
