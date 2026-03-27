// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class AllAnimalsLocalCacheMutation: LocalCacheMutation {
  static let operationType: GraphQLOperationType = .query

  init() {}

  struct Data: TestSchema.MutableSelectionSet {
    var __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("allAnimals", [AllAnimal].self),
    ] }

    var allAnimals: [AllAnimal] {
      get { __data["allAnimals"] }
      set { __data["allAnimals"] = newValue }
    }

    init(
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
    ///
    /// Parent Type: `RenamedAnimal`
    struct AllAnimal: TestSchema.MutableSelectionSet {
      var __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("species", String.self),
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .inlineFragment(AsBird.self),
      ] }

      var species: String {
        get { __data["species"] }
        set { __data["species"] = newValue }
      }
      var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? {
        get { __data["skinCovering"] }
        set { __data["skinCovering"] = newValue }
      }

      var asBird: AsBird? {
        get { _asInlineFragment() }
        set { if let newData = newValue?.__data._data { __data._data = newData }}
      }

      init(
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
      ///
      /// Parent Type: `Bird`
      struct AsBird: TestSchema.MutableInlineFragment {
        var __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsLocalCacheMutation.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Bird }
        static var __selections: [Apollo.Selection] { [
          .field("wingspan", Double.self),
        ] }

        var wingspan: Double {
          get { __data["wingspan"] }
          set { __data["wingspan"] = newValue }
        }
        var species: String {
          get { __data["species"] }
          set { __data["species"] = newValue }
        }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? {
          get { __data["skinCovering"] }
          set { __data["skinCovering"] = newValue }
        }

        init(
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
