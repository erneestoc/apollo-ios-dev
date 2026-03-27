// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class PetSearchLocalCacheMutation: LocalCacheMutation {
  public static let operationType: GraphQLOperationType = .query

  public var filters: GraphQLNullable<TestSchema.PetSearchFilters>

  public init(filters: GraphQLNullable<TestSchema.PetSearchFilters> = .init(
    TestSchema.PetSearchFilters(
      species: ["Dog", "Cat"],
      size: .init(.SMALL),
      measurements: .init(
        TestSchema.MeasurementsInput(
          height: 10.5,
          weight: 5.0
        )
      )
    )
  )) {
    self.filters = filters
  }

  public var __variables: GraphQLOperation.Variables? { ["filters": filters] }

  public struct Data: TestSchema.MutableSelectionSet {
    public var __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Query }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("pets", [Pet].self, arguments: ["filters": .variable("filters")]),
    ] }

    public var pets: [Pet] {
      get { __data["pets"] }
      set { __data["pets"] = newValue }
    }

    public init(
      pets: [Pet]
    ) {
      self.init(_dataDict: DataDict(
        data: [
          "__typename": TestSchema.Objects.Query.typename,
          "pets": pets._fieldData,
        ],
        fulfilledFragments: [
          ObjectIdentifier(PetSearchLocalCacheMutation.Data.self)
        ]
      ))
    }

    /// Pet
    public struct Pet: TestSchema.MutableSelectionSet {
      public var __data: DataDict
      public init(_dataDict: DataDict) { __data = _dataDict }

      public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
      public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("humanName", String?.self),
      ] }

      public var id: TestSchema.ID {
        get { __data["id"] }
        set { __data["id"] = newValue }
      }
      public var humanName: String? {
        get { __data["humanName"] }
        set { __data["humanName"] = newValue }
      }

      public init(
        __typename: String,
        id: TestSchema.ID,
        humanName: String? = nil
      ) {
        self.init(_dataDict: DataDict(
          data: [
            "__typename": __typename,
            "id": id,
            "humanName": humanName,
          ],
          fulfilledFragments: [
            ObjectIdentifier(PetSearchLocalCacheMutation.Data.Pet.self)
          ]
        ))
      }
    }
  }
}
